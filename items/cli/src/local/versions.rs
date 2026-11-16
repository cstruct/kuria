//! Versions in local mode (`cli.local.versions`): kept in the directory's
//! files, read back from git.

use serde_json::{Value as Json, json};

use kuria_engine::markdown::{self, Target, TargetVersion};
use kuria_engine::versions::{self, Pin};

use super::answers::markdown_of;
use super::dir::{AktDir, LocalAkt, read_text};
use super::git::Times;
use super::write::Writer;
use crate::error::CliError;
use crate::output::Answer;

/// The number of a version, as a directory's file keeps it.
#[must_use]
pub fn number_of(version: &Json) -> i64 {
    version["number"].as_i64().unwrap_or_default()
}

/// The pins `version` records.
#[must_use]
pub fn pins_of(version: &Json) -> Vec<Pin> {
    serde_json::from_value(version["pins"].clone()).unwrap_or_default()
}

/// What the akt `number` has for a reference to it: its headings now, and
/// each version's, which its entry keeps.
#[must_use]
pub fn target(dir: &AktDir, number: &str) -> Option<Target> {
    let akt = dir.any(number)?;
    let ty = &dir.type_of(akt).ty;
    Some(Target {
        headings: markdown::heading_ids(ty, &markdown_of(dir, akt)),
        versions: akt
            .versions
            .iter()
            .map(|v| TargetVersion {
                number: u32::try_from(number_of(v)).unwrap_or_default(),
                revision: v["revision"]
                    .as_i64()
                    .and_then(|r| i32::try_from(r).ok())
                    .unwrap_or_default(),
                headings: v["headings"]
                    .as_array()
                    .map(|h| {
                        h.iter()
                            .filter_map(|i| i.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
                picture: v["picture"] == true,
            })
            .collect(),
        revision: akt.revision,
        image: ty.document().presentation.image.is_some(),
        picture: has_picture(dir, akt),
        versioned: ty.document().versions.is_some(),
    })
}

/// Whether `akt` is an image with a picture: a file of the directory in
/// the field its type names as `image` (`akt.field.markdown.images`).
// r[impl cli.local.attachments]
#[must_use]
pub fn has_picture(dir: &AktDir, akt: &LocalAkt) -> bool {
    dir.type_of(akt)
        .ty
        .document()
        .presentation
        .image
        .as_ref()
        .and_then(|key| akt.values.get(key))
        .and_then(Json::as_str)
        .is_some_and(|path| {
            super::attachments::holder(path).is_some() && dir.root.join(path).is_file()
        })
}

/// `kuria release` in local mode: each akt's current revision, committed,
/// as its next version, all or nothing, once none of their references is
/// broken; a reference from one to another read as what that one is
/// released as (`version.together`).
// r[impl cli.local.versions]
// r[impl version.release]
// r[impl version.references]
// r[impl version.pins]
// r[impl version.together]
pub fn release(
    w: &mut Writer,
    times: &Times,
    numbers: &[String],
    label: Option<&str>,
) -> Result<Answer, CliError> {
    let label = label_of(label)?;
    let mut releasing = Vec::new();
    for (i, number) in numbers.iter().enumerate() {
        if numbers[..i].contains(number) {
            return Err(CliError::refused(
                "BAD_REQUEST",
                format!("{number} is named twice"),
            ));
        }
        releasing.push(releasable(w, times, number)?);
    }
    // Each akt released here as it is released, at the version it gets.
    let dir = &w.dir;
    let mut group = std::collections::HashMap::new();
    for (akt, ty, fields, next) in &releasing {
        if let Some(mut found) = target(dir, &akt.number) {
            found.versions.push(TargetVersion {
                number: u32::try_from(*next).unwrap_or_default(),
                revision: akt.revision,
                headings: markdown::heading_ids(ty, fields),
                picture: has_picture(dir, akt),
            });
            group.insert(akt.number.clone(), found);
        }
    }
    let targets = |n: &str| group.get(n).cloned().or_else(|| target(dir, n));
    let mut pins = Vec::new();
    let mut broken = Vec::new();
    let mut together: Vec<String> = Vec::new();
    for (akt, ty, fields, _) in &releasing {
        match versions::check(ty, fields, &targets) {
            Ok(found) => pins.push(found),
            Err(found) => {
                for b in found {
                    broken.push(if releasing.len() > 1 {
                        format!("{}: {b}", akt.number)
                    } else {
                        b.to_string()
                    });
                    if let Some(number) = b.together
                        && !together.contains(&number)
                    {
                        together.push(number);
                    }
                }
            }
        }
    }
    if !broken.is_empty() {
        let hint = if together.is_empty() {
            String::new()
        } else {
            format!(
                " (released together with {}, they would be mended: `kuria release` them all at once)",
                together.join(", ")
            )
        };
        return Err(CliError::refused(
            "BROKEN_REFERENCES",
            format!(
                "the release has broken references: {}{hint}",
                broken.join("; ")
            ),
        ));
    }
    let mut lines = Vec::new();
    let mut released = Vec::new();
    for ((akt, ty, fields, next), pins) in releasing.into_iter().zip(pins) {
        let number = akt.number.clone();
        let mut version = json!({
            "number": next,
            "revision": akt.revision,
            "label": label,
            "by": w.ids.me.clone().unwrap_or_default(),
            "at": w.now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "pins": pins,
            "headings": markdown::heading_ids(&ty, &fields),
        });
        // Whether it had a picture, kept as its headings are, for an image
        // naming the version.
        if has_picture(&w.dir, &akt) {
            version["picture"] = true.into();
        }
        // A release makes no revision (`version.history`).
        w.akt_mut(&number).versions.push(version);
        lines.push(format!(
            "{number}: released version {next}{} (revision {}).",
            label.map(|l| format!(", {l}")).unwrap_or_default(),
            akt.revision
        ));
        released.push(
            json!({ "number": number, "version": next, "revision": akt.revision, "label": label }),
        );
    }
    let json = match <[Json; 1]>::try_from(released) {
        Ok([one]) => one,
        Err(all) => json!({ "released": all }),
    };
    Ok(super::write::answer(lines.join("\n"), json, None))
}

/// `label` trimmed, `None` if empty; refused unless one line of at most
/// 100 characters.
fn label_of(label: Option<&str>) -> Result<Option<&str>, CliError> {
    let label = label.map(str::trim).filter(|l| !l.is_empty());
    if label.is_some_and(|l| l.chars().count() > 100 || l.contains(['\n', '\r'])) {
        return Err(CliError::refused(
            "BAD_REQUEST",
            "a label is one line of at most 100 characters",
        ));
    }
    Ok(label)
}

/// `number`, checked as releasable: its type releases versions, git has
/// it committed, and its revision isn't a version yet. Answers it, its
/// type, its fields and the version it gets.
fn releasable(
    w: &Writer,
    times: &Times,
    number: &str,
) -> Result<
    (
        LocalAkt,
        kuria_config::AktType,
        std::collections::BTreeMap<String, kuria_engine::Value>,
        i64,
    ),
    CliError,
> {
    let akt = w.akt(number)?.clone();
    let ty = w.dir.type_of(&akt).ty.clone();
    if ty.document().versions.is_none() {
        return Err(CliError::refused(
            "BAD_REQUEST",
            format!("`{}` akter don't release versions", ty.key()),
        ));
    }
    if times.dirty(&akt) {
        return Err(CliError::refused(
            "UNCOMMITTED",
            format!("{number} has changes git hasn't committed: commit them, then release"),
        ));
    }
    if akt.versions.iter().any(|v| v["revision"] == akt.revision) {
        return Err(CliError::refused(
            "BAD_REQUEST",
            format!("revision {} of {number} is already a version", akt.revision),
        ));
    }
    let fields = markdown_of(&w.dir, &akt);
    let next = akt.versions.iter().map(number_of).max().unwrap_or(0) + 1;
    Ok((akt, ty, fields, next))
}

/// The akt `number` as it was at its version `n`, read from git, with the
/// version; `None` if it has no such version.
// r[impl version.read]
pub fn at(
    dir: &AktDir,
    times: &Times,
    number: &str,
    n: i64,
) -> Result<Option<(LocalAkt, Json)>, CliError> {
    let Some(akt) = dir.akt(number) else {
        return Ok(None);
    };
    let Some(version) = akt.versions.iter().find(|v| number_of(v) == n).cloned() else {
        return Ok(None);
    };
    let revision = version["revision"]
        .as_i64()
        .and_then(|r| i32::try_from(r).ok())
        .unwrap_or_default();
    let Some((path, text)) = times.text_at(number, revision) else {
        return Err(CliError::refused(
            "NOT_FOUND",
            format!("git has no commit of {number} at revision {revision}"),
        ));
    };
    let found = read_text(&dir.root.join(path), text, &dir.types)
        .into_iter()
        .filter_map(Result::ok)
        .find(|a| a.number == number);
    Ok(found.map(|mut read| {
        read.versions.clone_from(&akt.versions);
        (read, version)
    }))
}

/// An akt's versions, latest first, as GraphQL answers them.
#[must_use]
pub fn listed(akt: &LocalAkt) -> Vec<Json> {
    let mut listed: Vec<Json> = akt.versions.iter().map(shown).collect();
    listed.reverse();
    listed
}

/// One version as GraphQL answers it.
#[must_use]
pub fn shown(version: &Json) -> Json {
    let by = version["by"].as_str().unwrap_or_default();
    json!({
        "number": version["number"],
        "revision": version["revision"],
        "label": version["label"],
        "releasedBy": { "id": by, "name": by, "email": by.contains('@').then_some(by) },
        "releasedAt": version["at"],
        "pins": version["pins"],
    })
}
