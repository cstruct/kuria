//! Attachments in an akt directory (`cli.local.attachments`): each a file
//! `attachments/<number>/<name>`, which the akt file's value names by that
//! path from the directory.

use std::path::Path;

use kuria_config::{AttachmentFormat, FieldDef};
use kuria_engine::sbom;

/// Where an akt directory keeps its attachments.
pub const DIRECTORY: &str = "attachments";

/// Media types by file extension, the first of a media type's being the
/// one a file of it is given.
const EXTENSIONS: [(&str, &str); 22] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("svg", "image/svg+xml"),
    ("webp", "image/webp"),
    ("gif", "image/gif"),
    ("pdf", "application/pdf"),
    ("json", "application/json"),
    ("xml", "application/xml"),
    ("zip", "application/zip"),
    ("gz", "application/gzip"),
    ("tar", "application/x-tar"),
    ("txt", "text/plain"),
    ("log", "text/plain"),
    ("md", "text/markdown"),
    ("csv", "text/csv"),
    ("html", "text/html"),
    ("yaml", "application/yaml"),
    ("yml", "application/yaml"),
    (
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    (
        "xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    ("bin", "application/octet-stream"),
];

/// The media type of the file `name`, by its extension.
#[must_use]
pub fn media_type(name: &str) -> &'static str {
    let extension = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    EXTENSIONS
        .iter()
        .find(|(e, _)| *e == extension)
        .map_or("application/octet-stream", |(_, t)| t)
}

/// The extension a file of `media_type` is given, if there is one.
#[must_use]
pub fn extension(media_type: &str) -> Option<&'static str> {
    EXTENSIONS
        .iter()
        .find(|(_, t)| *t == media_type)
        .map(|(e, _)| *e)
}

/// `name` as a file name of its own: no directories, not hidden, and with
/// an extension saying `media_type` if it has one.
#[must_use]
pub fn file_name(name: &str, media_type: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_start_matches('.');
    let name = if name.is_empty() { "attachment" } else { name };
    match extension(media_type) {
        Some(e) if self::media_type(name) != media_type => format!("{name}.{e}"),
        _ => name.to_owned(),
    }
}

/// The `n`th name for a file `name`, for when another file has it: `name`
/// itself, then `<stem>-2.<extension>`, ….
#[must_use]
pub fn numbered(name: &str, n: usize) -> String {
    if n <= 1 {
        return name.to_owned();
    }
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem}-{n}.{extension}"),
        _ => format!("{name}-{n}"),
    }
}

/// The akt whose directory of attachments `path` is in, if it is
/// `attachments/<number>/<name>`.
#[must_use]
pub fn holder(path: &str) -> Option<&str> {
    let mut parts = path.strip_prefix(&format!("{DIRECTORY}/"))?.split('/');
    let (number, name) = (parts.next()?, parts.next()?);
    let plain = |p: &str| !p.is_empty() && p != "." && p != ".." && !p.contains('\\');
    (parts.next().is_none() && plain(number) && plain(name)).then_some(number)
}

/// Why `bytes`, the file `name`, can't be held by `field`, if they can't:
/// a media type, by its extension, that the field doesn't take
/// (`attachment.media-types`), or no SBOM in a field of SBOMs
/// (`attachment.sbom`).
#[must_use]
pub fn unfit(field: &FieldDef, name: &str, bytes: &[u8]) -> Option<String> {
    let media_type = media_type(name);
    if !field.media_types.is_empty() && !field.media_types.iter().any(|t| t == media_type) {
        return Some(format!(
            "`{name}` is {media_type}, and the field takes {}",
            field.media_types.join(", ")
        ));
    }
    if field.format == Some(AttachmentFormat::Sbom) && sbom::detect(bytes).is_none() {
        return Some(format!("`{name}` isn't an SBOM (SPDX or CycloneDX JSON)"));
    }
    None
}

/// Why the value `path` can't be held by `field` in the directory at
/// `root`, if it can't: it doesn't name a file under
/// `attachments/<number>/`, the file isn't there, or it doesn't fit.
// r[impl cli.local.attachments]
#[must_use]
pub fn problem(root: &Path, field: &FieldDef, path: &str) -> Option<String> {
    if holder(path).is_none() {
        return Some(format!(
            "`{path}` isn't a file of the directory's `{DIRECTORY}/<number>/<name>`"
        ));
    }
    match std::fs::read(root.join(path)) {
        Ok(bytes) => unfit(field, path, &bytes),
        Err(_) => Some(format!("the directory has no file `{path}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_and_their_media_types() {
        assert_eq!(media_type("attachments/IMG-1/Logo.PNG"), "image/png");
        assert_eq!(media_type("sbom.spdx.json"), "application/json");
        assert_eq!(media_type("README"), "application/octet-stream");
        assert_eq!(file_name("logo.png", "image/png"), "logo.png");
        assert_eq!(file_name("logo", "image/png"), "logo.png");
        assert_eq!(file_name("../../x.svg", "image/svg+xml"), "-..-x.svg");
        assert_eq!(file_name(".hidden", "application/x-unknown"), "hidden");
        assert_eq!(numbered("logo.png", 1), "logo.png");
        assert_eq!(numbered("logo.png", 2), "logo-2.png");
        assert_eq!(numbered("LICENSE", 3), "LICENSE-3");
        assert_eq!(holder("attachments/IMG-1/logo.png"), Some("IMG-1"));
        for path in [
            "attachments/logo.png",
            "attachments/IMG-1/../logo.png",
            "attachments/IMG-1/a/b.png",
            "elsewhere/IMG-1/logo.png",
            "attachments/../x/y",
        ] {
            assert_eq!(holder(path), None, "{path}");
        }
    }
}
