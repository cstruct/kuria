//! `kuria git-credential` (`git.repo.serve`): a git credential helper giving
//! git the CLI's sign-in for the repositories Kuria hosts, so that
//! `git -c credential.helper='!kuria git-credential' clone
//! https://kuria.example/git/DOCS-1.git` works.
//!
//! git asks with `get`, `store` or `erase`, and a description of what it
//! wants on standard input, one `key=value` per line. Kuria takes the access
//! token as the password of HTTP Basic authentication.

use std::collections::BTreeMap;
use std::io::Read;

use serde_json::json;

use crate::error::CliError;
use crate::output::Answer;
use crate::session::{Context, Session};

/// The user name the CLI gives git: any would do, Kuria reads the token.
pub const USERNAME: &str = "kuria";

/// What git describes, as `key=value` lines up to a blank line.
#[must_use]
pub fn asked(input: &str) -> BTreeMap<String, String> {
    input
        .lines()
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// Whether git asks for the Kuria at `url`: the same scheme, host and port.
#[must_use]
pub fn is_for(asked: &BTreeMap<String, String>, url: &str) -> bool {
    let Ok(url) = url::Url::parse(url) else {
        return false;
    };
    let host = match url.port() {
        Some(port) => format!("{}:{port}", url.host_str().unwrap_or_default()),
        None => url.host_str().unwrap_or_default().to_owned(),
    };
    asked.get("protocol").map(String::as_str) == Some(url.scheme())
        && asked
            .get("host")
            .is_some_and(|h| h.eq_ignore_ascii_case(&host))
}

/// What `get` answers git: the user name and the token, as git reads them.
#[must_use]
pub fn credentials(token: &str) -> String {
    format!("username={USERNAME}\npassword={token}\n")
}

/// Answers git's `action`, reading what it describes from standard input.
// r[impl git.repo.serve]
pub async fn run(context: &Context, action: &str) -> Result<Answer, CliError> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| CliError::unavailable(e.to_string()))?;
    let nothing = Answer {
        markdown: String::new(),
        json: json!({}),
    };
    // Kuria keeps the sign-in; what git would store or erase isn't the CLI's.
    if action != "get" {
        return Ok(nothing);
    }
    let asked = asked(&input);
    let Ok(url) = context.url() else {
        return Ok(nothing);
    };
    if !is_for(&asked, url) {
        return Ok(nothing);
    }
    let session = Session::open(context).await?;
    Ok(Answer {
        markdown: credentials(session.access_token()),
        json: json!({ "username": USERNAME }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify git.repo.serve]
    #[test]
    fn answers_git_for_its_kuria_only() {
        let asked = asked("protocol=https\nhost=kuria.example\npath=git/DOCS-1.git\n\nignored=1\n");
        assert_eq!(asked["path"], "git/DOCS-1.git");
        assert!(!asked.contains_key("ignored"));
        assert!(is_for(&asked, "https://kuria.example"));
        assert!(!is_for(&asked, "http://kuria.example"));
        assert!(!is_for(&asked, "https://other.example"));
        let local = super::asked("protocol=http\nhost=127.0.0.1:8080\n");
        assert!(is_for(&local, "http://127.0.0.1:8080"));
        assert!(!is_for(&local, "http://127.0.0.1:9090"));
        assert_eq!(credentials("t0k"), "username=kuria\npassword=t0k\n");
    }
}
