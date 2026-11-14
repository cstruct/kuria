//! Local mode (`cli.local.*`): an akt directory of Markdown files in place
//! of a Kuria server.
//!
//! - [`dir`]: loading `kuria.toml`, `types/` and `akter/`, and `init`.
//! - [`answers`]: what Kuria's GraphQL would answer for the CLI's reads,
//!   built from the directory, so every read renders as it does against a
//!   server.
//! - [`git`]: when files changed, and their history.
//! - [`check`]: the whole directory checked, every problem named.
//! - [`write`], [`edits`] and [`arrange`]: writes, through the engine's
//!   rules, under the directory's lock; [`values`]: values to and from the
//!   engine's, people as text.

pub mod answers;
pub mod arrange;
pub mod check;
pub mod dir;
pub mod edits;
pub mod git;
pub mod values;
pub mod write;

pub use dir::{AktDir, init};

use crate::error::CliError;

/// The refusal for what local mode doesn't do.
// r[impl cli.local.unsupported]
#[must_use]
pub fn unsupported(what: &str, why: &str) -> CliError {
    CliError::refused(
        "LOCAL_UNSUPPORTED",
        format!("{what} isn't available in local mode: {why}"),
    )
}
