//! Releasing a version (`version.*`): the check that no reference is
//! broken, and what each reference naming no version is pinned to.

use std::collections::BTreeMap;

use kuria_config::{AktType, FieldKind};
use serde::{Deserialize, Serialize};

use crate::Value;
use crate::markdown::{self, Reference, Target};

/// What a reference naming no version meant when its akt was released
/// (`version.pins`): its target's latest version, else its revision then.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    /// As the text says it: `kuria:<number>[#<heading>]`.
    pub destination: String,
    pub number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<i32>,
}

/// A reference that is broken, in which field, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Broken {
    pub field: String,
    pub destination: String,
    pub why: String,
    /// The target, if releasing it together would mend the reference: it
    /// releases versions, and has now what its latest version lacks.
    pub together: Option<String>,
}

impl std::fmt::Display for Broken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "`{}` in `{}`: {}",
            self.destination, self.field, self.why
        )
    }
}

/// Every reference in the `markdown` fields of an akt of `ty` with
/// `fields`, each with its field, in the type's field order.
#[must_use]
pub fn references_of(ty: &AktType, fields: &BTreeMap<String, Value>) -> Vec<(String, Reference)> {
    ty.document()
        .fields
        .iter()
        .filter(|f| f.kind == FieldKind::Markdown)
        .filter_map(|f| match fields.get(&f.key) {
            Some(Value::Markdown(text)) => Some((f.key.clone(), markdown::references(text))),
            _ => None,
        })
        .flat_map(|(field, references)| references.into_iter().map(move |r| (field.clone(), r)))
        .collect()
}

/// Every image embedded in the `markdown` fields of an akt of `ty` with
/// `fields`, each with its field, in the type's field order.
#[must_use]
pub fn images_of(ty: &AktType, fields: &BTreeMap<String, Value>) -> Vec<(String, Reference)> {
    ty.document()
        .fields
        .iter()
        .filter(|f| f.kind == FieldKind::Markdown)
        .filter_map(|f| match fields.get(&f.key) {
            Some(Value::Markdown(text)) => Some((f.key.clone(), markdown::images(text))),
            _ => None,
        })
        .flat_map(|(field, images)| images.into_iter().map(move |r| (field.clone(), r)))
        .collect()
}

/// Checks a release of an akt of `ty` with `fields`: none of its
/// references may be broken, `target` giving each referenced akt as the
/// releaser may see it, `None` if it doesn't exist or they may not read
/// it (`version.references`). Answers the pins its references naming no
/// version get (`version.pins`), or every broken reference.
// r[impl version.references]
// r[impl version.pins]
pub fn check(
    ty: &AktType,
    fields: &BTreeMap<String, Value>,
    target: &dyn Fn(&str) -> Option<Target>,
) -> Result<Vec<Pin>, Vec<Broken>> {
    let mut broken = Vec::new();
    let mut pins: Vec<Pin> = Vec::new();
    let images = images_of(ty, fields);
    for (field, reference) in references_of(ty, fields) {
        let found = target(&reference.number);
        let embedded = images.contains(&(field.clone(), reference.clone()));
        let check = |read: &Reference| {
            if embedded {
                markdown::broken_image(read, found.as_ref())
            } else {
                markdown::broken(read, found.as_ref())
            }
        };
        let why = match as_released(&reference, found.as_ref()) {
            Err(why) => Some(why),
            Ok(read) => check(&read),
        };
        if let Some(why) = why {
            let together = found
                .as_ref()
                .filter(|t| t.versioned && reference.version.is_none())
                .filter(|_| check(&reference).is_none())
                .map(|_| reference.number.clone());
            broken.push(Broken {
                field,
                destination: reference.to_string(),
                why,
                together,
            });
            continue;
        }
        let Some(found) = found else { continue };
        if reference.version.is_some()
            || pins.iter().any(|p| p.destination == reference.to_string())
        {
            continue;
        }
        let latest = found
            .versions
            .iter()
            .max_by_key(|v| v.number)
            .filter(|_| found.versioned);
        pins.push(Pin {
            destination: reference.to_string(),
            number: reference.number.clone(),
            version: latest.map(|v| v.number),
            revision: if latest.is_some() {
                None
            } else {
                Some(found.revision)
            },
        });
    }
    if broken.is_empty() {
        Ok(pins)
    } else {
        Err(broken)
    }
}

/// `reference` as a release checks it: one naming no version, to a target
/// whose type releases versions, as naming its latest version; refused if
/// it has none (`version.references`).
fn as_released(reference: &Reference, target: Option<&Target>) -> Result<Reference, String> {
    let Some(target) = target.filter(|t| t.versioned && reference.version.is_none()) else {
        return Ok(reference.clone());
    };
    match target.versions.iter().map(|v| v.number).max() {
        Some(latest) => Ok(Reference {
            version: Some(latest),
            ..reference.clone()
        }),
        None => Err(format!(
            "`{}` has no version yet: release it first, or together",
            reference.number
        )),
    }
}

/// `reference` as a version reads it: one naming no version, as its pin
/// says, if it has one (`version.read`).
#[must_use]
pub fn as_pinned(reference: &Reference, pins: &[Pin]) -> (Option<u32>, Option<i32>) {
    if reference.version.is_some() {
        return (reference.version, None);
    }
    match pins.iter().find(|p| p.destination == reference.to_string()) {
        Some(pin) => (pin.version, pin.revision),
        None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::markdown::TargetVersion;

    fn ty() -> AktType {
        kuria_config::validate(
            serde_json::from_value(json!({
                "key": "document", "name": "Document", "number": "D-{seq}",
                "fields": [
                    { "key": "title", "name": "Title", "kind": "text", "required": true },
                    { "key": "body", "name": "Body", "kind": "markdown" }
                ],
                "states": [{ "key": "draft", "name": "Draft", "initial": true }],
                "versions": {}
            }))
            .unwrap(),
        )
        .unwrap()
    }

    fn body(text: &str) -> BTreeMap<String, Value> {
        BTreeMap::from([("body".to_owned(), Value::Markdown(text.to_owned()))])
    }

    fn targets(number: &str) -> Option<Target> {
        match number {
            "D-1" => Some(Target {
                headings: vec!["install".to_owned(), "unreleased".to_owned()],
                versions: vec![
                    TargetVersion {
                        number: 1,
                        revision: 2,
                        headings: vec!["setup".to_owned()],
                        picture: false,
                    },
                    TargetVersion {
                        number: 2,
                        revision: 4,
                        headings: vec!["install".to_owned()],
                        picture: false,
                    },
                ],
                revision: 6,
                versioned: true,
                ..Default::default()
            }),
            "D-3" => Some(Target {
                headings: vec!["install".to_owned()],
                revision: 1,
                versioned: true,
                ..Default::default()
            }),
            "D-2" => Some(Target {
                headings: Vec::new(),
                versions: Vec::new(),
                revision: 3,
                ..Default::default()
            }),
            "IMG-1" => Some(Target {
                revision: 2,
                image: true,
                picture: true,
                ..Default::default()
            }),
            "IMG-2" => Some(Target {
                revision: 1,
                image: true,
                ..Default::default()
            }),
            _ => None,
        }
    }

    // r[verify version.references]
    // r[verify version.pins]
    #[test]
    fn a_release_pins_what_its_references_meant() {
        let pins = check(
            &ty(),
            &body("[a](kuria:D-1#install) [b](kuria:D-1@1#setup) [c](kuria:D-2) [again](kuria:D-1#install)"),
            &targets,
        )
        .unwrap();
        assert_eq!(
            pins,
            [
                Pin {
                    destination: "kuria:D-1#install".to_owned(),
                    number: "D-1".to_owned(),
                    version: Some(2),
                    revision: None
                },
                Pin {
                    destination: "kuria:D-2".to_owned(),
                    number: "D-2".to_owned(),
                    version: None,
                    revision: Some(3)
                },
            ]
        );
        let reference = Reference::parse("kuria:D-2").unwrap();
        assert_eq!(as_pinned(&reference, &pins), (None, Some(3)));
        assert_eq!(
            as_pinned(&Reference::parse("kuria:D-1@1").unwrap(), &pins),
            (Some(1), None)
        );
    }

    // r[verify version.references]
    #[test]
    fn a_release_is_refused_while_a_reference_is_broken() {
        let broken = check(
            &ty(),
            &body("[a](kuria:D-1#gone) [b](kuria:D-1@3) [c](kuria:D-1@1#install) [d](kuria:D-9) [ok](kuria:D-2)"),
            &targets,
        )
        .unwrap_err();
        let said: Vec<String> = broken.iter().map(ToString::to_string).collect();
        assert_eq!(
            said,
            [
                "`kuria:D-1#gone` in `body`: `D-1` has no heading `gone` in version 2",
                "`kuria:D-1@3` in `body`: `D-1` has no version 3",
                "`kuria:D-1@1#install` in `body`: `D-1` has no heading `install` in version 1",
                "`kuria:D-9` in `body`: there is no akt `D-9`",
            ]
        );
    }

    // r[verify version.references]
    #[test]
    fn a_versioned_target_is_read_as_last_released() {
        let broken = check(
            &ty(),
            &body("[a](kuria:D-1#unreleased) [b](kuria:D-3#install) [c](kuria:D-3)"),
            &targets,
        )
        .unwrap_err();
        let said: Vec<String> = broken.iter().map(ToString::to_string).collect();
        assert_eq!(
            said,
            [
                "`kuria:D-1#unreleased` in `body`: `D-1` has no heading `unreleased` in version 2",
                "`kuria:D-3#install` in `body`: `D-3` has no version yet: release it first, or together",
                "`kuria:D-3` in `body`: `D-3` has no version yet: release it first, or together",
            ]
        );
        // Each is mended by releasing its target together: each has now
        // what the reference names.
        let together: Vec<Option<&str>> = broken.iter().map(|b| b.together.as_deref()).collect();
        assert_eq!(together, [Some("D-1"), Some("D-3"), Some("D-3")]);
        // A target whose type releases none is read as it is.
        assert!(check(&ty(), &body("[a](kuria:D-2)"), &targets).is_ok());
    }

    // r[verify akt.field.markdown.images]
    // r[verify version.references]
    #[test]
    fn embedded_images_are_checked_and_pinned_as_references() {
        let pins = check(&ty(), &body("![A diagram](kuria:IMG-1)"), &targets).unwrap();
        assert_eq!(
            pins,
            [Pin {
                destination: "kuria:IMG-1".to_owned(),
                number: "IMG-1".to_owned(),
                version: None,
                revision: Some(2)
            }]
        );
        let broken = check(
            &ty(),
            &body("![none](kuria:IMG-2) ![doc](kuria:D-2) ![h](kuria:IMG-1#x) ![gone](kuria:IMG-9) [linked](kuria:D-2)"),
            &targets,
        )
        .unwrap_err();
        let said: Vec<&str> = broken.iter().map(|b| b.why.as_str()).collect();
        assert_eq!(
            said,
            [
                "`IMG-2` has no picture",
                "`D-2` isn't an image",
                "`kuria:IMG-1#x` names a heading, which an embedded image can't",
                "there is no akt `IMG-9`",
            ]
        );
    }
}
