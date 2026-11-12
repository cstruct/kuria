//! The hash of an akt's state and field values.

use std::collections::BTreeMap;

use serde_json::Value as Json;
use sha2::{Digest, Sha256};

use crate::value::{Value, fields_to_json};

/// The SHA-256 of `{"fields": …, "state": …}` as canonical JSON: no
/// whitespace, and the members of every object in key order.
///
/// It doesn't depend on how `serde_json` orders maps, which a Cargo feature
/// elsewhere in the build could change.
// r[impl akt.revision.record]
#[must_use]
pub fn state_hash(state: &str, fields: &BTreeMap<String, Value>) -> [u8; 32] {
    let document = Json::Object(
        [
            ("fields".to_owned(), Json::Object(fields_to_json(fields))),
            ("state".to_owned(), Json::String(state.to_owned())),
        ]
        .into_iter()
        .collect(),
    );
    let mut canonical = String::new();
    write_canonical(&mut canonical, &document);
    Sha256::digest(canonical.as_bytes()).into()
}

fn write_canonical(out: &mut String, json: &Json) {
    match json {
        Json::Object(object) => {
            let mut members: Vec<_> = object.iter().collect();
            members.sort_by(|a, b| a.0.cmp(b.0));
            out.push('{');
            for (i, (key, value)) in members.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Json::String(key.clone()).to_string());
                out.push(':');
                write_canonical(out, value);
            }
            out.push('}');
        }
        Json::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(out, item);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify akt.revision.record]
    #[test]
    fn hashes_a_known_document() {
        let fields = BTreeMap::from([
            ("title".to_owned(), Value::Text("Ä".to_owned())),
            (
                "evidence".to_owned(),
                Value::Reference {
                    url: "https://example.com".to_owned(),
                    label: Some("PR".to_owned()),
                },
            ),
        ]);
        let mut canonical = String::new();
        write_canonical(
            &mut canonical,
            &serde_json::json!({
                "state": "open",
                "fields": { "title": "Ä", "evidence": { "url": "https://example.com", "label": "PR" } },
            }),
        );
        assert_eq!(
            canonical,
            r#"{"fields":{"evidence":{"label":"PR","url":"https://example.com"},"title":"Ä"},"state":"open"}"#
        );
        assert_eq!(
            state_hash("open", &fields),
            <[u8; 32]>::from(Sha256::digest(canonical.as_bytes()))
        );
        assert_ne!(state_hash("closed", &fields), state_hash("open", &fields));
    }
}
