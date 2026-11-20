//! Verdicts: each assignee's `pass`, `fail` or `skip` of an akt, in a
//! `verdict` field (`akt.field.verdict`).

use chrono::{DateTime, Utc};
use serde_json::{Value as Json, json};
use uuid::Uuid;

use crate::value::{Change, Someone};

/// An assignee's judgement of an akt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Verdict {
    Pass,
    Fail,
    Skip,
}

impl Verdict {
    /// Reads `pass`, `fail` or `skip`.
    pub fn read(text: &str) -> Result<Self, String> {
        match text {
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            "skip" => Ok(Self::Skip),
            other => Err(format!("`{other}` isn't a verdict: pass, fail or skip")),
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Skip => "skip",
        }
    }
}

/// A verdict given: by whom, when, and for which assignee, who is someone
/// else than the giver when it was given for them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Given {
    pub assignee: Someone,
    pub verdict: Verdict,
    /// What the assignee saw (`akt.field.verdict.description`).
    pub description: Option<String>,
    pub by: Uuid,
    pub at: DateTime<Utc>,
}

impl Given {
    /// Whether someone other than the assignee gave it.
    #[must_use]
    pub fn for_another(&self) -> bool {
        self.assignee != Someone::User(self.by)
    }

    pub(crate) fn to_json(&self) -> Json {
        let mut json = json!({
            "assignee": self.assignee.to_json(),
            "verdict": self.verdict.as_str(),
            "by": self.by.to_string(),
            "at": self.at.to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
        });
        // Only when there is one, so a verdict without reads as before.
        if let Some(description) = &self.description {
            json["description"] = json!(description);
        }
        json
    }

    pub(crate) fn from_json(json: &Json) -> Result<Self, String> {
        let object =
            crate::value::members(json, &["assignee", "verdict", "description", "by", "at"])?;
        let assignee = Someone::from_json(
            object
                .get("assignee")
                .ok_or("a verdict names its assignee")?,
        )?;
        let verdict = Verdict::read(
            object
                .get("verdict")
                .and_then(Json::as_str)
                .unwrap_or_default(),
        )?;
        let by = object
            .get("by")
            .and_then(Json::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or("a verdict says who gave it")?;
        let at = object
            .get("at")
            .and_then(Json::as_str)
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .ok_or("a verdict says when it was given")?;
        let description = match object.get("description") {
            None | Some(Json::Null) => None,
            Some(Json::String(text)) => Some(text.clone()),
            Some(_) => return Err("a verdict's description is text".to_owned()),
        };
        Ok(Self {
            assignee,
            verdict,
            description,
            by,
            at: at.with_timezone(&Utc),
        })
    }
}

/// Every verdict of a stored `verdict` field, one per assignee.
pub(crate) fn all_from_json(json: &Json) -> Result<Vec<Given>, String> {
    let items = json.as_array().ok_or("expected a list of verdicts")?;
    let mut given: Vec<Given> = Vec::with_capacity(items.len());
    for item in items {
        let one = Given::from_json(item)?;
        if given.iter().any(|g| g.assignee == one.assignee) {
            return Err("an assignee has one verdict at most".to_owned());
        }
        given.push(one);
    }
    Ok(given)
}

/// What a request does to a `verdict` field: give `verdict` for `assignee`,
/// with `description` if any, or take theirs back if `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Giving {
    pub assignee: Someone,
    pub verdict: Option<Verdict>,
    pub description: Option<String>,
}

impl Giving {
    /// Reads a request's input: `pass`, `fail` or `skip` for oneself,
    /// `{"verdict": …, "description": …, "for": <user>}` with a description
    /// or for another assignee, and `{"clear": true}`, with `for` if for
    /// another, to take one back. A fail or a skip without a description is
    /// refused.
    // r[impl akt.field.verdict]
    // r[impl akt.field.verdict.description]
    pub fn from_input(json: &Json, change: Change) -> Result<Self, String> {
        let me = Someone::User(change.by);
        if let Some(text) = json.as_str() {
            return Self {
                assignee: me,
                verdict: Some(Verdict::read(text)?),
                description: None,
            }
            .described();
        }
        let object = crate::value::members(json, &["verdict", "description", "for", "clear"]).map_err(|_| {
            "expected pass, fail or skip, {\"verdict\": …, \"description\": …, \"for\": <user>} or {\"clear\": true}".to_owned()
        })?;
        let assignee = object
            .get("for")
            .map(Someone::from_json)
            .transpose()?
            .unwrap_or(me);
        let verdict = match (object.get("verdict"), object.get("clear")) {
            (Some(v), None) => Some(Verdict::read(
                v.as_str().ok_or("a verdict is pass, fail or skip")?,
            )?),
            (None, Some(Json::Bool(true))) => None,
            _ => return Err("give a `verdict` or `clear: true`".to_owned()),
        };
        let description = match object.get("description") {
            None | Some(Json::Null) => None,
            Some(Json::String(text)) if verdict.is_some() => {
                Some(text.trim().to_owned()).filter(|t| !t.is_empty())
            }
            Some(Json::String(_)) => {
                return Err("a verdict taken back has no description".to_owned());
            }
            Some(_) => return Err("a verdict's description is text".to_owned()),
        };
        Self {
            assignee,
            verdict,
            description,
        }
        .described()
    }

    /// This, unless it gives a fail or a skip without saying why.
    fn described(self) -> Result<Self, String> {
        match self.verdict {
            Some(v @ (Verdict::Fail | Verdict::Skip)) if self.description.is_none() => Err(
                format!("a `{}` says why: give it a `description`", v.as_str()),
            ),
            _ => Ok(self),
        }
    }

    /// `given` with this applied, among `assignees`, by `change`: a verdict
    /// from or for anyone else is refused, and giving the same verdict with
    /// the same description again keeps who gave it first.
    // r[impl akt.field.verdict]
    // r[impl akt.field.verdict.description]
    pub fn apply(
        &self,
        given: &[Given],
        assignees: &[Someone],
        change: Change,
    ) -> Result<Vec<Given>, String> {
        if !assignees.contains(&self.assignee) {
            return Err(if self.assignee == Someone::User(change.by) {
                "only an assignee gives a verdict; give one for an assignee with `for`".to_owned()
            } else {
                "a verdict is only given for an assignee".to_owned()
            });
        }
        let mut given: Vec<Given> = given.to_vec();
        let at = given.iter().position(|g| g.assignee == self.assignee);
        match (self.verdict, at) {
            (None, Some(i)) => {
                given.remove(i);
            }
            (None, None) => {}
            (Some(v), Some(i))
                if given[i].verdict == v && given[i].description == self.description => {}
            (Some(verdict), at) => {
                let one = Given {
                    assignee: self.assignee,
                    verdict,
                    description: self.description.clone(),
                    by: change.by,
                    at: change.at,
                };
                match at {
                    Some(i) => given[i] = one,
                    None => given.push(one),
                }
            }
        }
        Ok(given)
    }
}

/// `given` without the verdicts of anyone no longer among `assignees`.
// r[impl akt.field.verdict]
#[must_use]
pub fn only_of(given: &[Given], assignees: &[Someone]) -> Vec<Given> {
    given
        .iter()
        .filter(|g| assignees.contains(&g.assignee))
        .cloned()
        .collect()
}
