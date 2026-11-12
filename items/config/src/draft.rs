//! Draft recipes: how a field is drafted from the akter around its akt.

use serde::{Deserialize, Serialize};

/// A field's draft recipe:
///
/// ```json
/// { "through": ["release_candidate", "hotfix"], "depth": 2,
///   "types": ["epic", "backlog_item"], "line": "{number} {field:title}",
///   "group_by": "type" }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftDef {
    /// The akt types of the containers to look inside; any if empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub through: Vec<String>,
    /// How many levels of members to look through.
    #[serde(default = "one")]
    pub depth: u8,
    /// The akt types to list.
    pub types: Vec<String>,
    /// The text of each listed akt.
    pub line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<GroupBy>,
}

/// How drafted lines are grouped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    /// Under a heading per akt type.
    Type,
}

fn one() -> u8 {
    1
}

/// The deepest a recipe may look.
pub const MAX_DEPTH: u8 = 5;

/// A parsed line template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineTemplate {
    parts: Vec<LinePart>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LinePart {
    Literal(String),
    Number,
    Type,
    State,
    Field(String),
}

/// What a line is rendered from.
pub struct LineValues<'a> {
    pub number: &'a str,
    /// The akt type's name.
    pub akt_type: &'a str,
    /// The state's name.
    pub state: &'a str,
    /// A field's value as text, or empty.
    pub field: &'a dyn Fn(&str) -> String,
}

impl LineTemplate {
    /// Parses `line`, or says why it is invalid.
    // r[impl draft.line]
    pub fn parse(line: &str) -> Result<Self, String> {
        let mut parts = Vec::new();
        let mut rest = line;
        while let Some(i) = rest.find(['{', '}']) {
            if i > 0 {
                parts.push(LinePart::Literal(rest[..i].to_owned()));
            }
            if rest[i..].starts_with('}') {
                return Err("a `}` has no `{`".to_owned());
            }
            let after = &rest[i + 1..];
            let end = after.find('}').ok_or("a `{` is never closed")?;
            let name = &after[..end];
            parts.push(match name {
                "number" => LinePart::Number,
                "type" => LinePart::Type,
                "state" => LinePart::State,
                other => match other.strip_prefix("field:") {
                    Some(key) if !key.is_empty() && !key.contains('{') => {
                        LinePart::Field(key.to_owned())
                    }
                    _ => {
                        return Err(format!(
                            "`{{{other}}}` is not a placeholder; use {{number}}, {{type}}, {{state}} or {{field:<key>}}"
                        ));
                    }
                },
            });
            rest = &after[end + 1..];
        }
        if !rest.is_empty() {
            parts.push(LinePart::Literal(rest.to_owned()));
        }
        Ok(Self { parts })
    }

    /// The line for `values`.
    #[must_use]
    pub fn render(&self, values: &LineValues<'_>) -> String {
        let mut out = String::new();
        for part in &self.parts {
            match part {
                LinePart::Literal(text) => out.push_str(text),
                LinePart::Number => out.push_str(values.number),
                LinePart::Type => out.push_str(values.akt_type),
                LinePart::State => out.push_str(values.state),
                LinePart::Field(key) => out.push_str(&(values.field)(key)),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify draft.line]
    #[test]
    fn renders_placeholders() {
        let line = LineTemplate::parse("- {number} ({type}, {state}): {field:title}").unwrap();
        let field = |key: &str| {
            if key == "title" {
                "Fix login".to_owned()
            } else {
                String::new()
            }
        };
        let rendered = line.render(&LineValues {
            number: "ITEM-7",
            akt_type: "Backlog item",
            state: "Done",
            field: &field,
        });
        assert_eq!(rendered, "- ITEM-7 (Backlog item, Done): Fix login");
    }

    // r[verify draft.line]
    #[test]
    fn refuses_invalid_lines() {
        for line in ["{title}", "{field:}", "{number", "number}", "{field:a{b}"] {
            assert!(LineTemplate::parse(line).is_err(), "{line}");
        }
    }
}
