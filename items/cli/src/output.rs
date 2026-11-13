//! Answers in Markdown or JSON, and refusals on standard error.

use std::io::Write as _;

use clap::ValueEnum;
use serde_json::Value;

use crate::error::CliError;

/// How a command answers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Readable as the app shows the same things.
    #[default]
    Md,
    /// Stable objects for scripts and agents.
    Json,
}

/// What a command answers: the same thing in both formats.
#[derive(Debug)]
pub struct Answer {
    pub markdown: String,
    pub json: Value,
}

impl Answer {
    // r[impl cli.format]
    pub fn print(&self, format: Format) {
        let mut out = std::io::stdout().lock();
        let written = match format {
            Format::Md => writeln!(out, "{}", self.markdown.trim_end()),
            Format::Json => writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&self.json).expect("serializable")
            ),
        };
        // A closed pipe, such as `| head`, isn't the command failing.
        drop(written);
    }
}

/// Says `error` on standard error, in `format`.
pub fn refusal(error: &CliError, format: Format) {
    match format {
        Format::Md => eprintln!("error: {error}"),
        Format::Json => eprintln!("{}", error.json()),
    }
}

/// A note for the person, on standard error, so it never mixes with an
/// answer a script reads.
pub fn note(text: &str) {
    eprintln!("{text}");
}
