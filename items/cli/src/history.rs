//! An akt's history as the app says it (`app.history`): who did what,
//! when, what it changed, the users and contacts by name, and memberships
//! and links by number.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value as Json, json};

/// The part of a query asking for an akt's revisions.
pub const REVISIONS: &str = "revisions {
  revision transition reason changes membership createdAt
  by { name }
  named { __typename ... on User { id name } ... on Contact { id displayName } }
}";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub revision: i64,
    pub transition: Option<String>,
    pub reason: Option<String>,
    pub changes: Json,
    pub membership: Option<Json>,
    pub created_at: String,
    pub by: By,
    #[serde(default)]
    pub named: Vec<Named>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct By {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "__typename", rename_all_fields = "camelCase")]
pub enum Named {
    User { id: String, name: String },
    Contact { id: String, display_name: String },
}

/// A revision as people read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Described {
    pub revision: i64,
    pub who: String,
    pub at: String,
    /// "performed Release", "created it", "edited it", "changed its links"
    /// or "changed its containers or members".
    pub what: String,
    pub reason: Option<String>,
    /// Each field's name and the value it was set to; `None` when cleared.
    pub changes: Vec<(String, Option<String>)>,
    pub memberships: Vec<String>,
}

/// The longest text a change shows before it is shortened.
const LONG: usize = 140;

fn someone(who: &Json, names: &HashMap<String, String>) -> String {
    if let Some(id) = who.as_str() {
        return names
            .get(id)
            .cloned()
            .unwrap_or_else(|| "a user".to_owned());
    }
    who["contact"]
        .as_str()
        .and_then(|id| names.get(id).cloned())
        .unwrap_or_else(|| "a contact".to_owned())
}

/// A stored value as short text, users and contacts by `names`.
#[must_use]
pub(crate) fn value_text(value: &Json, names: &HashMap<String, String>) -> Option<String> {
    Some(match value {
        Json::Null => return None,
        Json::String(text) if uuid::Uuid::parse_str(text).is_ok() => someone(value, names),
        Json::String(text) => {
            let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if flat.chars().count() > LONG {
                format!("{}…", flat.chars().take(LONG - 1).collect::<String>())
            } else {
                flat
            }
        }
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        Json::Array(items) => items
            .iter()
            .map(|v| value_text(v, names).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(", "),
        Json::Object(o) => {
            let s = |k: &str| o.get(k).and_then(Json::as_str);
            if let Some(by) = o.get("checked_by") {
                match o.get("for") {
                    Some(f) if f != by => {
                        format!(
                            "checked for {} by {}",
                            someone(f, names),
                            someone(by, names)
                        )
                    }
                    _ => format!("checked by {}", someone(by, names)),
                }
            } else if o.contains_key("contact") {
                someone(value, names)
            } else if let Some(url) = s("url") {
                s("label").unwrap_or(url).to_owned()
            } else if o.contains_key("digest") {
                format!(
                    "{}{}",
                    s("repository").unwrap_or_default(),
                    s("tag").map(|t| format!(":{t}")).unwrap_or_default()
                )
            } else if let Some(repository) = s("repository") {
                format!(
                    "{repository}{}",
                    s("tag").map(|t| format!("@{t}")).unwrap_or_default()
                )
            } else {
                value.to_string()
            }
        }
    })
}

fn membership_words(kind: &str) -> Option<(&'static str, bool)> {
    Some(match kind {
        "added" => ("Added to", false),
        "removed" => ("Removed from", false),
        "added_member" => ("Added", false),
        "removed_member" => ("Removed", false),
        "linked" | "linked_from" => ("Linked:", true),
        "unlinked" | "unlinked_from" => ("Unlinked:", true),
        _ => return None,
    })
}

/// One revision, with the akt type's field and transition names.
// r[impl app.history]
#[must_use]
pub(crate) fn describe(
    r: &Revision,
    field_names: &HashMap<String, String>,
    transition_names: &HashMap<String, String>,
) -> Described {
    let names: HashMap<String, String> = r
        .named
        .iter()
        .map(|n| match n {
            Named::User { id, name } => (id.clone(), name.clone()),
            Named::Contact { id, display_name } => (id.clone(), display_name.clone()),
        })
        .collect();
    let changes: Vec<(String, Option<String>)> = r
        .changes
        .as_object()
        .into_iter()
        .flatten()
        .map(|(key, value)| {
            (
                field_names.get(key).cloned().unwrap_or_else(|| key.clone()),
                value_text(value, &names),
            )
        })
        .collect();
    let membership: Vec<(&String, &Json)> = r
        .membership
        .as_ref()
        .and_then(Json::as_object)
        .into_iter()
        .flatten()
        .collect();
    let mut memberships = Vec::new();
    for (kind, entries) in &membership {
        for entry in entries.as_array().into_iter().flatten() {
            let number = entry["number"].as_str().unwrap_or_default();
            memberships.push(match membership_words(kind) {
                Some((words, true)) => format!(
                    "{words} {} {number}",
                    entry["name"].as_str().unwrap_or_default()
                )
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
                Some((words, false)) => format!("{words} {number}"),
                None => format!("{kind} {number}"),
            });
        }
    }
    let only_links = !membership.is_empty()
        && membership
            .iter()
            .all(|(kind, _)| membership_words(kind).is_some_and(|(_, link)| link));
    let what = if let Some(t) = &r.transition {
        format!("performed {}", transition_names.get(t).unwrap_or(t))
    } else if r.revision == 1 {
        "created it".to_owned()
    } else if only_links && changes.is_empty() {
        "changed its links".to_owned()
    } else if !memberships.is_empty() && changes.is_empty() {
        "changed its containers or members".to_owned()
    } else {
        "edited it".to_owned()
    };
    Described {
        revision: r.revision,
        who: r.by.name.clone(),
        at: r.created_at.clone(),
        what,
        reason: r.reason.clone(),
        changes,
        memberships,
    }
}

impl Described {
    /// The revision as a Markdown list item.
    #[must_use]
    pub fn markdown(&self) -> String {
        let mut text = format!(
            "- **{}** · {} {} · {}",
            self.revision,
            self.who,
            self.what,
            when(&self.at)
        );
        if let Some(reason) = &self.reason {
            text.push_str(&format!(" — {reason}"));
        }
        text.push('\n');
        for (field, value) in &self.changes {
            match value {
                Some(value) => text.push_str(&format!("  - {field}: {value}\n")),
                None => text.push_str(&format!("  - {field}: cleared\n")),
            }
        }
        for membership in &self.memberships {
            text.push_str(&format!("  - {membership}\n"));
        }
        text
    }

    #[must_use]
    pub fn json(&self) -> Json {
        json!({
            "revision": self.revision,
            "by": self.who,
            "at": self.at,
            "what": self.what,
            "reason": self.reason,
            "changes": self.changes.iter().map(|(field, value)| json!({ "field": field, "value": value })).collect::<Vec<_>>(),
            "memberships": self.memberships,
        })
    }
}

/// A time as `YYYY-MM-DD HH:MM`, in the zone Kuria answered it in (UTC).
#[must_use]
pub fn when(at: &str) -> String {
    match at.get(..16) {
        Some(minutes) => format!("{} UTC", minutes.replace('T', " ")),
        None => at.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn revision(value: Json) -> Revision {
        serde_json::from_value(value).unwrap()
    }

    fn names(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    }

    // r[verify app.history]
    #[test]
    fn says_revisions_as_the_app_does() {
        let fields = names(&[("assignee", "Assignee"), ("ok", "Approved")]);
        let transitions = names(&[("start", "Start")]);
        let dev = "01900000-0000-7000-8000-0000000000aa";
        let created = revision(json!({
            "revision": 1, "transition": null, "reason": null, "membership": null,
            "changes": { "assignee": dev, "ok": { "checked_by": dev, "for": { "contact": "c1" } } },
            "createdAt": "2026-11-13T14:05:06.123Z", "by": { "name": "Dev" },
            "named": [
                { "__typename": "User", "id": dev, "name": "Dev" },
                { "__typename": "Contact", "id": "c1", "displayName": "Auditor" }
            ]
        }));
        let described = describe(&created, &fields, &transitions);
        assert_eq!(described.what, "created it");
        assert_eq!(
            described.changes,
            [
                ("Assignee".to_owned(), Some("Dev".to_owned())),
                (
                    "Approved".to_owned(),
                    Some("checked for Auditor by Dev".to_owned())
                )
            ]
        );
        assert_eq!(
            described.markdown(),
            "- **1** · Dev created it · 2026-11-13 14:05 UTC\n  - Assignee: Dev\n  - Approved: checked for Auditor by Dev\n"
        );

        let linked = revision(json!({
            "revision": 2, "transition": null, "reason": "Related", "changes": {},
            "membership": { "linked_from": [{ "number": "TASK-1", "name": "Blocked by" }] },
            "createdAt": "2026-11-13T14:06:00Z", "by": { "name": "Dev" }
        }));
        let described = describe(&linked, &fields, &transitions);
        assert_eq!(described.what, "changed its links");
        assert_eq!(described.memberships, ["Linked: Blocked by TASK-1"]);

        let started = revision(json!({
            "revision": 3, "transition": "start", "reason": null, "changes": { "assignee": null },
            "membership": null, "createdAt": "2026-11-13T14:07:00Z", "by": { "name": "Dev" }
        }));
        let described = describe(&started, &fields, &transitions);
        assert_eq!(described.what, "performed Start");
        assert_eq!(described.changes, [("Assignee".to_owned(), None)]);

        let added = revision(json!({
            "revision": 4, "transition": null, "reason": null, "changes": {},
            "membership": { "added": [{ "number": "BACKLOG-1" }] },
            "createdAt": "2026-11-13T14:08:00Z", "by": { "name": "Dev" }
        }));
        assert_eq!(
            describe(&added, &fields, &transitions).what,
            "changed its containers or members"
        );
    }

    #[test]
    fn shortens_long_text() {
        let long = Json::String("word ".repeat(100));
        let text = value_text(&long, &HashMap::new()).unwrap();
        assert_eq!(text.chars().count(), LONG);
        assert!(text.ends_with('…'));
    }
}
