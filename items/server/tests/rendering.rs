//! Rendering in a process of its own, with the server's own binary as the
//! worker (`render.limits`).

use std::time::{Duration, Instant};

use kuria_typst::{Job, Limits, RenderError, Renderer};

/// A diagram that computes for minutes in little memory: 400 million
/// steps, which typst neither caps, as it does `while` loops, nor caches, as
/// it does pure calls.
const ENDLESS: &str = "#let s = 0\n#for i in range(20000) { for j in range(20000) { s += j } }\n#s";

fn renderer(limits: Limits) -> Renderer {
    Renderer {
        program: env!("CARGO_BIN_EXE_kuria-server").into(),
        limits,
    }
}

fn diagram(source: &str) -> Job {
    Job::Diagram {
        source: source.to_owned(),
    }
}

// r[verify render.limits]
#[tokio::test(flavor = "multi_thread")]
async fn a_render_is_done_in_its_own_process() {
    let renderer = renderer(Limits::default());
    let svg = renderer
        .render(diagram("#diagram(node((0, 0), [A]))"))
        .await
        .unwrap();
    assert!(svg.starts_with(b"<svg"));
    let pdf = renderer
        .render(Job::Pdf {
            markup: "= Hello\n\n#image(\"/a.svg\")".to_owned(),
            files: vec![(
                "/a.svg".to_owned(),
                b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"/>".to_vec(),
            )],
        })
        .await
        .unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let refused = renderer.render(diagram("#diagram(")).await.unwrap_err();
    assert!(
        matches!(refused, RenderError::Refused(ref m) if !m.is_empty()),
        "{refused:?}"
    );
}

// r[verify render.limits]
#[tokio::test(flavor = "multi_thread")]
async fn a_render_past_its_time_is_stopped() {
    let started = Instant::now();
    let stopped = renderer(Limits {
        wall: Duration::from_secs(2),
        ..Limits::default()
    })
    .render(diagram(ENDLESS))
    .await
    .unwrap_err();
    assert_eq!(
        stopped,
        RenderError::Limit("took longer than 2 s".to_owned())
    );
    assert!(started.elapsed() < Duration::from_secs(10));
}

// r[verify render.limits]
#[tokio::test(flavor = "multi_thread")]
async fn a_render_past_its_cpu_time_is_stopped() {
    let stopped = renderer(Limits {
        cpu_seconds: 1,
        ..Limits::default()
    })
    .render(diagram(ENDLESS))
    .await
    .unwrap_err();
    assert_eq!(
        stopped,
        RenderError::Limit("used more than 1 s of CPU time".to_owned())
    );
}

// r[verify render.limits]
#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread")]
async fn a_render_past_its_memory_is_stopped() {
    let stopped = renderer(Limits {
        memory_bytes: 256 * 1024 * 1024,
        ..Limits::default()
    })
    .render(diagram("#let a = range(100000000)\n#a.len()"))
    .await
    .unwrap_err();
    assert_eq!(
        stopped,
        RenderError::Limit("used more than 256 MB of memory".to_owned())
    );
}
