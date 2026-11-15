//! Pictures in a clone (`git.pictures`): each image a document embeds is a
//! file `attachments/<number>.<extension>`, and the document links to it by
//! a path relative to its own file, so that editors and git hosts show it.
//! Kuria keeps `kuria:<number>`; these turn one into the other.

use std::collections::HashMap;

use kuria_engine::markdown::{Reference, image_destinations, images};

/// Where a clone keeps its pictures.
pub const DIRECTORY: &str = "attachments";

/// The file a picture of `media_type` is kept in, for the image `number`.
#[must_use]
pub fn path_of(number: &str, media_type: &str) -> String {
    let extension = match media_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/svg+xml" => "svg",
        "image/webp" => "webp",
        _ => "bin",
    };
    format!("{DIRECTORY}/{number}.{extension}")
}

/// The image a path in the clone is the picture of, if it is one.
#[must_use]
pub fn number_of(path: &str) -> Option<String> {
    let name = path.strip_prefix(&format!("{DIRECTORY}/"))?;
    let (number, extension) = name.rsplit_once('.')?;
    (!number.is_empty() && !number.contains('/') && !extension.contains('/'))
        .then(|| number.to_owned())
}

/// `to`, a path from the clone's root, as seen from the file at `from`.
#[must_use]
pub fn relative(from: &str, to: &str) -> String {
    let depth = from.matches('/').count();
    format!("{}{to}", "../".repeat(depth))
}

/// `to`, a path relative to the file at `from`, from the clone's root;
/// `None` if it leaves the clone.
#[must_use]
pub fn resolve(from: &str, to: &str) -> Option<String> {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in to.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// The images `text` embeds that a clone shows as pictures: those naming
/// an akt only, no version or heading.
#[must_use]
pub fn embedded(text: &str) -> Vec<String> {
    images(text)
        .into_iter()
        .filter(|r| r.version.is_none() && r.heading.is_none())
        .map(|r| r.number)
        .collect()
}

/// `text`, the file at `path`, with each embedded image that has a picture
/// in `pictures` (image number to picture path) linked to it relatively.
#[must_use]
pub fn linked_relatively<S: std::hash::BuildHasher>(
    text: &str,
    path: &str,
    pictures: &HashMap<String, String, S>,
) -> String {
    let mut out = text.to_owned();
    for (range, destination) in image_destinations(text).into_iter().rev() {
        let Some(reference) = Reference::parse(&destination) else {
            continue;
        };
        if reference.version.is_some() || reference.heading.is_some() {
            continue;
        }
        if let Some(picture) = pictures.get(&reference.number) {
            out.replace_range(range, &relative(path, picture));
        }
    }
    out
}

/// `text`, the file at `path`, with each image linked to a picture in the
/// clone linked to its image in Kuria, `kuria:<number>`, as Kuria keeps it.
#[must_use]
pub fn linked_to_kuria(text: &str, path: &str) -> String {
    let mut out = text.to_owned();
    for (range, destination) in image_destinations(text).into_iter().rev() {
        if destination.contains(':') {
            continue;
        }
        if let Some(number) = resolve(path, &destination).as_deref().and_then(number_of) {
            out.replace_range(range, &format!("kuria:{number}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify git.pictures]
    #[test]
    fn links_pictures_relatively_and_back() {
        let pictures = HashMap::from([("IMG-1".to_owned(), "attachments/IMG-1.png".to_owned())]);
        let text =
            "![Chart](kuria:IMG-1) ![v](kuria:IMG-1@2) ![none](kuria:IMG-9) [link](kuria:IMG-1)";
        let nested = linked_relatively(text, "guides/deep/D-2-guide.md", &pictures);
        assert_eq!(
            nested,
            "![Chart](../../attachments/IMG-1.png) ![v](kuria:IMG-1@2) ![none](kuria:IMG-9) [link](kuria:IMG-1)"
        );
        assert_eq!(linked_to_kuria(&nested, "guides/deep/D-2-guide.md"), text);
        let top = linked_relatively(text, "D-1-index.md", &pictures);
        assert!(top.starts_with("![Chart](attachments/IMG-1.png)"), "{top}");
        assert_eq!(linked_to_kuria(&top, "D-1-index.md"), text);
        // A path outside the pictures, or out of the clone, stays as written.
        let other = "![x](images/a.png) ![y](../../../attachments/IMG-1.png)";
        assert_eq!(linked_to_kuria(other, "D-1-index.md"), other);
    }

    #[test]
    fn names_pictures_by_image_and_media_type() {
        assert_eq!(path_of("IMG-1", "image/svg+xml"), "attachments/IMG-1.svg");
        assert_eq!(number_of("attachments/IMG-1.svg").as_deref(), Some("IMG-1"));
        assert_eq!(number_of("attachments/x/IMG-1.svg"), None);
        assert_eq!(number_of("guides/IMG-1.svg"), None);
        assert_eq!(resolve("a/b.md", "../c/d.png").as_deref(), Some("c/d.png"));
        assert_eq!(resolve("b.md", "../d.png"), None);
        assert_eq!(embedded("![a](kuria:IMG-1) ![b](kuria:IMG-2#x)"), ["IMG-1"]);
    }
}
