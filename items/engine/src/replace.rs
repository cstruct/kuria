//! Replacements (`akt.replace`): spans of an akt's text and `markdown`
//! fields and what they become, found in the fields as they are when the
//! change is applied, not as their author read them.
//!
//! [`replace`] answers the field values an edit then writes, so that the
//! replacements are checked as an edit's and, for a live draft, written as
//! the least change to each text.

use std::collections::BTreeMap;

use kuria_config::{AktType, FieldKind};
use serde_json::{Map, Value as Json};

use crate::{Current, Refusal, Value};

/// One replacement: `span`, which must occur exactly once in `field`,
/// becomes `text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replacement {
    pub field: String,
    pub span: String,
    pub text: String,
}

/// The field values that `replacements` make of `current`'s fields: each
/// field they name, as a string, or `null` if it is left empty. Refuses a
/// span that isn't found, or is found more than once, in its field as
/// `current` holds it, a field that isn't text or `markdown`, an empty
/// span, spans of one field that overlap, and an akt in a final state
/// (`akt.replace.match`).
// r[impl akt.replace]
// r[impl akt.replace.match]
pub fn replace(
    ty: &AktType,
    current: &Current,
    replacements: &[Replacement],
) -> Result<Map<String, Json>, Refusal> {
    crate::refuse_final(ty, current)?;
    let mut by_field: BTreeMap<&str, Vec<&Replacement>> = BTreeMap::new();
    for replacement in replacements {
        by_field
            .entry(replacement.field.as_str())
            .or_default()
            .push(replacement);
    }
    if by_field.is_empty() {
        return Err(Refusal::BadReplacement {
            field: String::new(),
            reason: "give at least one replacement".to_owned(),
        });
    }
    let mut values = Map::new();
    for (key, replacements) in by_field {
        let bad = |reason: &str| Refusal::BadReplacement {
            field: key.to_owned(),
            reason: reason.to_owned(),
        };
        let field = ty
            .document()
            .fields
            .iter()
            .find(|f| f.key == key)
            .ok_or_else(|| bad("the akt type has no such field"))?;
        if !matches!(field.kind, FieldKind::Text | FieldKind::Markdown) {
            return Err(bad("only text and markdown fields take replacements"));
        }
        let before = match current.fields.get(key) {
            Some(Value::Text(text) | Value::Markdown(text)) => text.as_str(),
            _ => "",
        };
        let mut spans = Vec::with_capacity(replacements.len());
        for replacement in replacements {
            if replacement.span.is_empty() {
                return Err(bad("a span can't be empty"));
            }
            let found = occurrences(before, &replacement.span);
            match found.as_slice() {
                [start] => spans.push((*start, *start + replacement.span.len(), replacement)),
                [] => {
                    return Err(Refusal::NoMatch {
                        field: key.to_owned(),
                    });
                }
                more => {
                    return Err(Refusal::Ambiguous {
                        field: key.to_owned(),
                        count: more.len(),
                    });
                }
            }
        }
        spans.sort_by_key(|(start, _, _)| *start);
        if spans.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(bad("spans of one field can't overlap"));
        }
        let mut after = String::with_capacity(before.len());
        let mut at = 0;
        for (start, end, replacement) in spans {
            after.push_str(&before[at..start]);
            after.push_str(&replacement.text);
            at = end;
        }
        after.push_str(&before[at..]);
        let empty = match field.kind {
            FieldKind::Markdown => crate::markdown::is_empty(&after),
            _ => after.is_empty(),
        };
        values.insert(
            key.to_owned(),
            if empty {
                Json::Null
            } else {
                Json::String(after)
            },
        );
    }
    Ok(values)
}

/// Where `span` starts in `text`, every time, overlapping ones included.
fn occurrences(text: &str, span: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find(span) {
        let start = from + offset;
        found.push(start);
        // The next character boundary after `start`, so overlapping
        // occurrences count too.
        from = start + text[start..].chars().next().map_or(1, char::len_utf8);
        if from > text.len() {
            break;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use kuria_config::{AktTypeDocument, validate};
    use serde_json::json;

    use super::*;

    fn document() -> AktType {
        let document: AktTypeDocument = serde_json::from_value(json!({
            "key": "doc", "name": "Doc", "number": "D-{seq}",
            "fields": [
                { "key": "title", "name": "Title", "kind": "text", "required": true },
                { "key": "body", "name": "Body", "kind": "markdown" },
                { "key": "notes", "name": "Notes", "kind": "text", "multiline": true },
                { "key": "due", "name": "Due", "kind": "date" }
            ],
            "states": [{ "key": "open", "name": "Open", "initial": true }]
        }))
        .unwrap();
        validate(document).unwrap()
    }

    fn current(body: &str) -> Current {
        Current {
            state: "open".to_owned(),
            fields: BTreeMap::from([
                ("title".to_owned(), Value::Text("Install".to_owned())),
                ("body".to_owned(), Value::Markdown(body.to_owned())),
            ]),
        }
    }

    fn one(field: &str, span: &str, text: &str) -> Replacement {
        Replacement {
            field: field.to_owned(),
            span: span.to_owned(),
            text: text.to_owned(),
        }
    }

    // r[verify akt.replace]
    #[test]
    fn a_span_found_once_is_replaced_and_the_rest_kept() {
        let values = replace(
            &document(),
            &current("# Install\n\nRun the installer.\n\nThen restart."),
            &[one("body", "Run the installer.", "Run `setup`.")],
        )
        .unwrap();
        assert_eq!(
            values["body"],
            json!("# Install\n\nRun `setup`.\n\nThen restart.")
        );
    }

    // r[verify akt.replace]
    #[test]
    fn spans_of_one_field_are_found_in_the_field_as_it_was() {
        let values = replace(
            &document(),
            &current("one two three"),
            &[one("body", "three", "3"), one("body", "one", "two")],
        )
        .unwrap();
        assert_eq!(values["body"], json!("two two 3"));
    }

    // r[verify akt.replace.match]
    #[test]
    fn a_missing_or_repeated_span_is_refused() {
        let ty = document();
        assert_eq!(
            replace(&ty, &current("a b a"), &[one("body", "c", "d")]),
            Err(Refusal::NoMatch {
                field: "body".to_owned()
            })
        );
        assert_eq!(
            replace(&ty, &current("a b a"), &[one("body", "a", "d")]),
            Err(Refusal::Ambiguous {
                field: "body".to_owned(),
                count: 2
            })
        );
        assert_eq!(
            replace(&ty, &current("aaa"), &[one("body", "aa", "b")]),
            Err(Refusal::Ambiguous {
                field: "body".to_owned(),
                count: 2
            })
        );
        // An empty field holds no span.
        assert!(matches!(
            replace(&ty, &current("x"), &[one("notes", "x", "y")]),
            Err(Refusal::NoMatch { .. })
        ));
    }

    // r[verify akt.replace.match]
    #[test]
    fn other_fields_empty_spans_and_overlaps_are_bad() {
        let ty = document();
        for replacements in [
            vec![one("due", "x", "y")],
            vec![one("nope", "x", "y")],
            vec![one("body", "", "y")],
            vec![one("body", "ab", "x"), one("body", "bc", "y")],
            vec![],
        ] {
            assert!(
                matches!(
                    replace(&ty, &current("abcd"), &replacements),
                    Err(Refusal::BadReplacement { .. })
                ),
                "{replacements:?}"
            );
        }
    }

    // r[verify akt.replace]
    #[test]
    fn a_field_left_empty_is_cleared_and_text_fields_take_them_too() {
        let ty = document();
        assert_eq!(
            replace(&ty, &current("gone"), &[one("body", "gone", "")]).unwrap()["body"],
            Json::Null
        );
        assert_eq!(
            replace(&ty, &current("x"), &[one("title", "Install", "Set up")]).unwrap()["title"],
            json!("Set up")
        );
        // Multibyte text is cut on character boundaries.
        assert_eq!(
            replace(&ty, &current("åäö åäö!"), &[one("body", "ö!", "o")]).unwrap()["body"],
            json!("åäö åäo")
        );
    }
}
