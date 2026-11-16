//! Rendering in a process of its own (`render.limits`): the worker that
//! renders one job under limits it sets itself, and the [`Renderer`] that
//! starts one per job and stops it past its time.
//!
//! The job travels on the worker's standard input as one line of JSON, the
//! [`Header`], followed by each file's bytes in its order. The worker
//! answers the PDF or SVG on standard output and exits 0, or typst's message
//! on standard error and exits [`REFUSED`].

use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::process::{ExitCode, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

/// The worker's exit code when typst refuses the job.
pub const REFUSED: u8 = 2;

/// What a render may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// How long it may run, start to end.
    pub wall: Duration,
    /// How much CPU time it may use.
    pub cpu_seconds: u64,
    /// How much memory it may map.
    pub memory_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            wall: Duration::from_secs(30),
            cpu_seconds: 20,
            memory_bytes: 1024 * 1024 * 1024,
        }
    }
}

/// One job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Job {
    /// Markup as a PDF, with the files it reads, by path.
    Pdf {
        markup: String,
        files: Vec<(String, Vec<u8>)>,
    },
    /// A diagram block's source as SVG.
    Diagram { source: String },
}

/// The job's first line on the worker's standard input.
#[derive(Debug, Serialize, Deserialize)]
struct Header {
    kind: String,
    text: String,
    /// Each file's path and length, its bytes following in this order.
    files: Vec<(String, usize)>,
    cpu_seconds: u64,
    memory_bytes: u64,
}

/// Why a render failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// typst refused it, saying why.
    #[error("{0}")]
    Refused(String),
    /// It went past one of its limits.
    #[error("it {0}")]
    Limit(String),
    /// The worker couldn't be run.
    #[error("the renderer failed: {0}")]
    Failed(String),
}

/// What starts a worker for each job: the program, run with `render`, and
/// the limits it renders under.
#[derive(Clone, Debug)]
pub struct Renderer {
    pub program: PathBuf,
    pub limits: Limits,
}

impl Renderer {
    /// This program as the worker, with `limits`.
    pub fn current(limits: Limits) -> std::io::Result<Self> {
        Ok(Self {
            program: std::env::current_exe()?,
            limits,
        })
    }

    /// `job` rendered by a worker of its own: the PDF's or SVG's bytes.
    // r[impl render.limits]
    pub async fn render(&self, job: Job) -> Result<Vec<u8>, RenderError> {
        let (kind, text, files) = match job {
            Job::Pdf { markup, files } => ("pdf", markup, files),
            Job::Diagram { source } => ("diagram", source, Vec::new()),
        };
        let header = Header {
            kind: kind.to_owned(),
            text,
            files: files.iter().map(|(p, b)| (p.clone(), b.len())).collect(),
            cpu_seconds: self.limits.cpu_seconds,
            memory_bytes: self.limits.memory_bytes,
        };
        let failed = |e: std::io::Error| RenderError::Failed(e.to_string());
        let mut child = tokio::process::Command::new(&self.program)
            .arg("render")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(failed)?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RenderError::Failed("no input".to_owned()))?;
        let mut input =
            serde_json::to_vec(&header).map_err(|e| RenderError::Failed(e.to_string()))?;
        input.push(b'\n');
        for (_, bytes) in &files {
            input.extend_from_slice(bytes);
        }
        let feed = async move {
            // A worker that stops early closes its input; what it said counts.
            let _ = stdin.write_all(&input).await;
            drop(stdin);
        };
        let run = async {
            feed.await;
            child.wait_with_output().await
        };
        let output = match tokio::time::timeout(self.limits.wall, run).await {
            Ok(output) => output.map_err(failed)?,
            Err(_) => {
                return Err(RenderError::Limit(format!(
                    "took longer than {} s",
                    self.limits.wall.as_secs()
                )));
            }
        };
        if output.status.success() {
            return Ok(output.stdout);
        }
        let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if output.status.code() == Some(i32::from(REFUSED)) {
            return Err(RenderError::Refused(said));
        }
        Err(RenderError::Limit(self.stopped(output.status, &said)))
    }

    /// Which limit stopped a worker that ended as `status` said.
    fn stopped(&self, status: std::process::ExitStatus, said: &str) -> String {
        use std::os::unix::process::ExitStatusExt;
        match status.signal() {
            Some(s)
                if s == nix::sys::signal::Signal::SIGXCPU as i32
                    || s == nix::sys::signal::Signal::SIGKILL as i32 =>
            {
                format!("used more than {} s of CPU time", self.limits.cpu_seconds)
            }
            _ if said.contains("memory allocation") || status.signal().is_some() => format!(
                "used more than {} MB of memory",
                self.limits.memory_bytes / (1024 * 1024)
            ),
            _ => format!("failed: {said}"),
        }
    }
}

/// Sets this process's limits: CPU time, and memory where the system lets
/// a process lower it. macOS refuses to lower a process's address space,
/// so there only time limits it; Linux, where Kuria runs, limits both.
fn limit(cpu_seconds: u64, memory_bytes: u64) -> nix::Result<()> {
    use nix::sys::resource::{Resource, setrlimit};
    // The soft limit sends SIGXCPU; the hard one, a second later, SIGKILL.
    setrlimit(Resource::RLIMIT_CPU, cpu_seconds, cpu_seconds + 1)?;
    if cfg!(target_os = "macos") {
        return Ok(());
    }
    setrlimit(Resource::RLIMIT_AS, memory_bytes, memory_bytes)
}

/// The worker: renders the job on standard input under the limits it
/// names, answering as the module says.
// r[impl render.limits]
#[must_use]
pub fn main() -> ExitCode {
    let refused = |message: &str| {
        let _ = writeln!(std::io::stderr(), "{message}");
        ExitCode::from(REFUSED)
    };
    let mut input = std::io::stdin().lock();
    let mut line = String::new();
    if input.read_line(&mut line).is_err() {
        return refused("no job");
    }
    let Ok(header) = serde_json::from_str::<Header>(&line) else {
        return refused("the job doesn't read");
    };
    if let Err(e) = limit(header.cpu_seconds, header.memory_bytes) {
        let _ = writeln!(std::io::stderr(), "the limits couldn't be set: {e}");
        return ExitCode::FAILURE;
    }
    let mut files = Vec::with_capacity(header.files.len());
    for (path, len) in header.files {
        let mut bytes = vec![0; len];
        if input.read_exact(&mut bytes).is_err() {
            return refused("a file is cut short");
        }
        files.push((path, bytes));
    }
    let rendered = match header.kind.as_str() {
        "pdf" => crate::pdf(header.text, files),
        "diagram" => crate::diagram(&header.text).map(String::into_bytes),
        other => Err(format!("no job `{other}`")),
    };
    match rendered {
        Ok(bytes) => {
            let mut out = std::io::stdout().lock();
            if out.write_all(&bytes).and_then(|()| out.flush()).is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(message) => refused(&message),
    }
}
