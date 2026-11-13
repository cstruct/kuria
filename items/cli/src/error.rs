//! Refusals and failures, and the exit codes they end a command with.

use std::fmt;
use std::process::ExitCode;

use serde_json::json;

/// What kind of refusal or failure ended a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Refused for the request's content: a bad value, an invalid change.
    Refused,
    /// Something named doesn't exist, or can't be read.
    NotFound,
    /// The akt changed since the revision the command named.
    Stale,
    /// Not signed in, or not permitted.
    NotPermitted,
    /// Kuria or Konklav couldn't be reached, or failed.
    Unavailable,
}

impl Kind {
    // r[impl cli.errors]
    #[must_use]
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Refused => 1,
            Self::NotFound => 2,
            Self::Stale => 3,
            Self::NotPermitted => 4,
            Self::Unavailable => 5,
        }
    }

    /// The kind a Kuria error `code` is.
    #[must_use]
    pub fn of_code(code: &str) -> Self {
        match code {
            "NOT_FOUND" | "UNKNOWN_AKT_TYPE" => Self::NotFound,
            "STALE" => Self::Stale,
            "FORBIDDEN" | "UNAUTHENTICATED" | "NOT_SIGNED_IN" => Self::NotPermitted,
            "UNAVAILABLE" | "INTERNAL" => Self::Unavailable,
            _ => Self::Refused,
        }
    }
}

/// A refusal or failure, as the client says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliError {
    pub kind: Kind,
    /// The server's code, such as `STALE`, or the client's own, such as
    /// `NOT_SIGNED_IN`.
    pub code: String,
    pub message: String,
    /// The field the refusal belongs to, if any.
    pub field: Option<String>,
}

impl CliError {
    pub fn new(kind: Kind, code: &str, message: impl Into<String>) -> Self {
        Self {
            kind,
            code: code.to_owned(),
            message: message.into(),
            field: None,
        }
    }

    /// A refusal for the request's content, with the client's own `code`.
    pub fn refused(code: &str, message: impl Into<String>) -> Self {
        Self::new(Kind::Refused, code, message)
    }

    /// Kuria or Konklav failing, or out of reach.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(Kind::Unavailable, "UNAVAILABLE", message)
    }

    pub fn not_signed_in(message: impl Into<String>) -> Self {
        Self::new(Kind::NotPermitted, "NOT_SIGNED_IN", message)
    }

    /// A refusal from Kuria, by its `code`.
    pub fn from_server(code: &str, message: impl Into<String>, field: Option<String>) -> Self {
        Self {
            kind: Kind::of_code(code),
            code: code.to_owned(),
            message: message.into(),
            field,
        }
    }

    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::from(self.kind.exit_code())
    }

    /// The error as `--format json` writes it to standard error.
    #[must_use]
    pub fn json(&self) -> serde_json::Value {
        json!({ "error": { "code": self.code, "message": self.message, "field": self.field } })
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.code)?;
        if let Some(field) = &self.field {
            write!(f, ", in the field `{field}`")?;
        }
        Ok(())
    }
}

impl std::error::Error for CliError {}

impl From<reqwest::Error> for CliError {
    fn from(error: reqwest::Error) -> Self {
        Self::unavailable(format!("couldn't reach the server: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cli.errors]
    #[test]
    fn exits_by_what_refused() {
        for (code, exit) in [
            ("INVALID_LINK", 1),
            ("BAD_REQUEST", 1),
            ("NOT_FOUND", 2),
            ("UNKNOWN_AKT_TYPE", 2),
            ("STALE", 3),
            ("FORBIDDEN", 4),
            ("NOT_SIGNED_IN", 4),
            ("UNAVAILABLE", 5),
        ] {
            assert_eq!(Kind::of_code(code).exit_code(), exit, "{code}");
        }
    }

    // r[verify cli.errors]
    #[test]
    fn says_the_code_message_and_field() {
        let error = CliError::from_server("BAD_REQUEST", "Not a date", Some("due".to_owned()));
        assert_eq!(
            error.to_string(),
            "Not a date (BAD_REQUEST), in the field `due`"
        );
        assert_eq!(
            error.json(),
            json!({ "error": { "code": "BAD_REQUEST", "message": "Not a date", "field": "due" } })
        );
    }
}
