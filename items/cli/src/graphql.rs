//! Kuria's GraphQL API, through a session.

use reqwest::{Method, StatusCode};
use serde_json::{Value, json};

use crate::error::{CliError, Kind};
use crate::http::{Body, json as read_json};
use crate::session::Session;

impl Session {
    /// `query` with `variables`, answering its `data`, or the first error
    /// Kuria refused it with.
    pub async fn graphql(&self, query: &str, variables: Value) -> Result<Value, CliError> {
        let url = format!("{}/graphql", self.url);
        let body = json!({ "query": query, "variables": variables });
        let response = self
            .http
            .send(
                Method::POST,
                &url,
                Some(self.access_token()),
                &Body::Json(&body),
            )
            .await?;
        let status = response.status();
        match status {
            StatusCode::UNAUTHORIZED => {
                return Err(CliError::new(
                    Kind::NotPermitted,
                    "UNAUTHENTICATED",
                    "Kuria didn't accept the session; run `kuria login` again",
                ));
            }
            StatusCode::FORBIDDEN => {
                return Err(CliError::new(
                    Kind::NotPermitted,
                    "FORBIDDEN",
                    "Kuria refused the session for this tenant",
                ));
            }
            s if s.is_server_error() => {
                return Err(CliError::unavailable(format!("Kuria failed ({s})")));
            }
            _ => {}
        }
        let answer = read_json(response, "Kuria").await?;
        answered(answer)
    }
}

/// `data`, or the first error as the client says it.
fn answered(mut answer: Value) -> Result<Value, CliError> {
    if let Some(error) = answer["errors"].as_array().and_then(|e| e.first()) {
        let code = error["extensions"]["code"]
            .as_str()
            .unwrap_or("BAD_REQUEST");
        let mut message = error["message"].as_str().unwrap_or(code).to_owned();
        // A refused akt type document lists each problem with its path.
        if let Some(problems) = error["extensions"]["problems"].as_array() {
            for problem in problems {
                message.push_str(&format!(
                    "\n- {}: {}",
                    problem["path"].as_str().unwrap_or_default(),
                    problem["message"].as_str().unwrap_or_default()
                ));
            }
        }
        // A release's refusal names the akter releasing together would
        // mend references to (`version.together`).
        if let Some(together) = error["extensions"]["together"]
            .as_array()
            .filter(|t| !t.is_empty())
        {
            let numbers: Vec<&str> = together.iter().filter_map(Value::as_str).collect();
            message.push_str(&format!(
                " (released together with {}, they would be mended: `kuria release` them all at once)",
                numbers.join(", ")
            ));
        }
        // A batch's refusal names its operation, counting from 1.
        if let Some(n) = error["extensions"]["operation"].as_u64() {
            message = format!("operation {n}: {message}");
        }
        let field = error["extensions"]["field"].as_str().map(str::to_owned);
        return Err(CliError::from_server(code, message, field));
    }
    Ok(answer["data"].take())
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cli.errors]
    #[test]
    fn says_kuria_s_first_error() {
        let refused = answered(json!({
            "errors": [{ "message": "Changed since", "extensions": { "code": "STALE", "currentRevision": 4 } }],
            "data": null
        }))
        .unwrap_err();
        assert_eq!(refused.kind, Kind::Stale);
        assert_eq!(refused.message, "Changed since");
        let batch = answered(json!({
            "errors": [{ "message": "Final", "extensions": { "code": "REFUSED", "operation": 2 } }],
            "data": null
        }))
        .unwrap_err();
        assert_eq!(batch.message, "operation 2: Final");
        assert_eq!(
            answered(json!({ "data": { "viewer": { "tenant": "t" } } })).unwrap(),
            json!({ "viewer": { "tenant": "t" } })
        );
    }
}
