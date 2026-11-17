//! `kuria docs import <folder>` (`cli.docs-import`): a document kept
//! elsewhere brought in with its history. `metadata.json` names its field
//! values, its draft and its old versions, each a PDF; the draft is
//! converted to Markdown with pandoc, its pictures made image akter. The
//! akt is created, each version released with its PDF, oldest first, and
//! the draft made its body.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::model::Shape;
use crate::output::Answer;
use crate::session::Session;
use crate::versions::Releasing;

/// One old version: its PDF, label and description.
#[derive(Clone, Debug)]
pub struct Version {
    pub label: Option<String>,
    pub description: Option<String>,
    pub file: PathBuf,
}

/// A picture the draft embeds that can be made an image akt: where the
/// draft points, its alt text, and its file.
#[derive(Clone, Debug)]
pub struct Picture {
    pub destination: String,
    pub alt: String,
    pub file: PathBuf,
}

/// The draft in Markdown, the pictures it embeds that can be made image
/// akter, and the directory pandoc extracted them to.
#[derive(Debug)]
pub struct Draft {
    pub markdown: String,
    pub pictures: Vec<Picture>,
    _extracted: Option<Scratch>,
}

/// What a folder holds, checked: its field values as `--set`s, the number
/// it keeps, its draft, its references captured, and its versions, oldest
/// first.
#[derive(Debug)]
pub struct Folder {
    pub sets: Vec<String>,
    pub number: Option<String>,
    pub draft: Option<Draft>,
    pub versions: Vec<Version>,
}

fn bad(message: impl Into<String>) -> CliError {
    CliError::refused("BAD_FILE", message)
}

/// Reads and checks `folder`, converting its draft and capturing its
/// references by `metadata.json`'s `references`, else `references`
/// (`cli.docs-import`).
// r[impl cli.docs-import]
pub fn read(
    folder: &Path,
    references: Option<&str>,
    padding: Option<&Padding>,
) -> Result<Folder, CliError> {
    let path = folder.join("metadata.json");
    let text =
        std::fs::read_to_string(&path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
    let Json::Object(metadata) =
        serde_json::from_str(&text).map_err(|e| bad(format!("{}: {e}", path.display())))?
    else {
        return Err(bad(format!("{} isn't a JSON object", path.display())));
    };
    let in_folder = |name: &Json, what: &str| -> Result<PathBuf, CliError> {
        let name = name
            .as_str()
            .ok_or_else(|| bad(format!("{what} names no file")))?;
        let file = folder.join(name);
        if file.is_file() {
            Ok(file)
        } else {
            Err(bad(format!("{what}: there is no {}", file.display())))
        }
    };
    let mut sets = Vec::new();
    let mut draft = None;
    let mut versions = Vec::new();
    let mut number = None;
    let mut pattern = references.map(str::to_owned);
    for (key, value) in &metadata {
        match key.as_str() {
            "number" => {
                let given = value
                    .as_str()
                    .ok_or_else(|| bad("`number` is the akt's number, as text"))?;
                number = Some(Padding::apply(padding, given));
            }
            "references" => {
                pattern = Some(
                    value
                        .as_str()
                        .ok_or_else(|| bad("`references` is a pattern, as text"))?
                        .to_owned(),
                );
            }
            "draft" => draft = Some(draft_of(&in_folder(value, "the draft")?, folder)?),
            "versions" => {
                let listed = value
                    .as_array()
                    .ok_or_else(|| bad("`versions` is a list of versions, oldest first"))?;
                for (i, v) in listed.iter().enumerate() {
                    let what = format!("version {}", i + 1);
                    let file = in_folder(&v["file"], &what)?;
                    let bytes = std::fs::read(&file)
                        .map_err(|e| bad(format!("{}: {e}", file.display())))?;
                    if !bytes.starts_with(b"%PDF") {
                        return Err(bad(format!("{what}: {} isn't a PDF", file.display())));
                    }
                    let text = |key: &str| v[key].as_str().map(str::to_owned);
                    versions.push(Version {
                        label: text("label"),
                        description: text("description"),
                        file,
                    });
                }
            }
            _ => sets.push(set_of(key, value)?),
        }
    }
    if let (Some(pattern), Some(draft)) = (&pattern, &mut draft) {
        draft.markdown = capture(&draft.markdown, &references_of(pattern)?, padding);
    }
    Ok(Folder {
        sets,
        number,
        draft,
        versions,
    })
}

/// `pattern`, compiled, refused unless it names an `id` group.
fn references_of(pattern: &str) -> Result<regex::Regex, CliError> {
    let compiled = regex::Regex::new(pattern)
        .map_err(|e| CliError::refused("BAD_REQUEST", format!("the references pattern: {e}")))?;
    if !compiled.capture_names().any(|n| n == Some("id")) {
        return Err(CliError::refused(
            "BAD_REQUEST",
            "the references pattern needs a group named `id`, the number referred to, such as `(?<id>FOOID-\\d{4})`",
        ));
    }
    Ok(compiled)
}

/// Where `markdown` is code or already a link or picture, which capturing
/// references leaves alone.
fn kept(markdown: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut fenced = false;
    let mut at = 0;
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if fenced || fence {
            ranges.push(at..at + line.len());
            if fence {
                fenced = !fenced;
            }
        }
        at += line.len();
    }
    let spans = regex::Regex::new(r"(`+)[^`]*?(`+)|!?\[[^\]]*\]\([^)]*\)|<[a-z]+:[^>]*>")
        .expect("a valid pattern");
    ranges.extend(spans.find_iter(markdown).map(|m| m.range()));
    ranges
}

/// `markdown` with each match of `references`, outside code and links, a
/// reference to the akt its `id` group names, read as its `label` group,
/// else its number (`cli.docs-import`).
// r[impl cli.docs-import]
fn capture(markdown: &str, references: &regex::Regex, padding: Option<&Padding>) -> String {
    let kept = kept(markdown);
    let mut out = String::with_capacity(markdown.len());
    let mut last = 0;
    for found in references.captures_iter(markdown) {
        let (Some(whole), Some(id)) = (found.get(0), found.name("id")) else {
            continue;
        };
        let range = whole.range();
        if range.is_empty()
            || kept
                .iter()
                .any(|k| k.start < range.end && range.start < k.end)
        {
            continue;
        }
        let id = Padding::apply(padding, id.as_str());
        let label = found
            .name("label")
            .map(|l| l.as_str().trim())
            .filter(|l| !l.is_empty())
            .unwrap_or(&id);
        let label = label.replace('[', "\\[").replace(']', "\\]");
        out.push_str(&markdown[last..range.start]);
        out.push_str(&format!("[{label}](kuria:{id})"));
        last = range.end;
    }
    out.push_str(&markdown[last..]);
    out
}

/// How `--pad-numbers` writes a number given or captured: as the type's
/// pattern gives it in `parent`, its digits zero-padded to their widths.
#[derive(Debug)]
pub struct Padding {
    pub pattern: kuria_config::NumberPattern,
    pub parent: Option<String>,
}

impl Padding {
    /// The pattern of `shape`, read in the first of `inside`.
    pub fn of(shape: &Shape, inside: &[String]) -> Result<Self, CliError> {
        let pattern = kuria_config::NumberPattern::parse(&shape.number_pattern)
            .map_err(|e| CliError::unavailable(format!("`{}`'s number pattern: {e}", shape.key)))?;
        Ok(Self {
            pattern,
            parent: inside.first().cloned(),
        })
    }

    /// `number` padded, if there is padding and it fits so; else as it is.
    // r[impl cli.docs-import]
    fn apply(padding: Option<&Self>, number: &str) -> String {
        padding
            .and_then(|p| p.pattern.padded(number, p.parent.as_deref()))
            .unwrap_or_else(|| number.to_owned())
    }
}

/// A field's value from `metadata.json` as `--set` writes it
/// (`cli.values`): text as it is, a list as its items joined by commas.
fn set_of(key: &str, value: &Json) -> Result<String, CliError> {
    let text = match value {
        Json::String(text) => text.clone(),
        Json::Number(n) => n.to_string(),
        Json::Bool(b) => b.to_string(),
        Json::Array(items) => items
            .iter()
            .map(|i| i.as_str().map_or_else(|| i.to_string(), str::to_owned))
            .collect::<Vec<_>>()
            .join(","),
        Json::Null | Json::Object(_) => {
            return Err(bad(format!(
                "`{key}` in metadata.json isn't a value to set"
            )));
        }
    };
    Ok(format!("{key}={text}"))
}

/// A Lua filter that unwraps figures and drops pictures' sizes, so that
/// pandoc writes pictures as Markdown, not as HTML: Kuria shows a picture
/// alone in its paragraph as a figure, captioned by its alt text.
const PLAIN_PICTURES: &str = "function Figure(figure)\n  return figure.content\nend\n\n\
function Image(image)\n  image.attributes = {}\n  return image\nend\n";

/// The draft at `file`: as it is for Markdown, its pictures read from
/// `folder`; else converted by pandoc to GitHub-flavoured Markdown, its
/// pictures extracted.
fn draft_of(file: &Path, folder: &Path) -> Result<Draft, CliError> {
    let extension = file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if matches!(extension.as_deref(), Some("md" | "markdown")) {
        let markdown =
            std::fs::read_to_string(file).map_err(|e| bad(format!("{}: {e}", file.display())))?;
        let pictures = pictures_in(&markdown, folder);
        return Ok(Draft {
            markdown,
            pictures,
            _extracted: None,
        });
    }
    let extracted = Scratch::dir()?;
    let filter = extracted.0.join("plain-pictures.lua");
    std::fs::write(&filter, PLAIN_PICTURES).map_err(|e| CliError::unavailable(e.to_string()))?;
    let ran = Command::new("pandoc")
        .arg(file)
        .args(["--to", "gfm", "--wrap", "none"])
        .arg(format!("--extract-media={}", extracted.0.display()))
        .arg(format!("--lua-filter={}", filter.display()))
        .output()
        .map_err(|e| {
            CliError::refused(
                "PANDOC_MISSING",
                format!(
                    "converting {} needs pandoc on the PATH ({e}): install it, or give the draft as Markdown",
                    file.display()
                ),
            )
        })?;
    if !ran.status.success() {
        return Err(CliError::refused(
            "PANDOC_FAILED",
            format!(
                "pandoc couldn't convert {}: {}",
                file.display(),
                String::from_utf8_lossy(&ran.stderr).trim()
            ),
        ));
    }
    let markdown = String::from_utf8_lossy(&ran.stdout).into_owned();
    let pictures = pictures_in(&markdown, folder);
    Ok(Draft {
        markdown,
        pictures,
        _extracted: Some(extracted),
    })
}

/// Every picture `markdown` embeds, `![alt](destination)`, with its alt
/// text and where it points, but Kuria's and those on the web.
fn embedded(markdown: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = markdown;
    while let Some(start) = rest.find("![") {
        rest = &rest[start + 2..];
        let Some(close) = rest.find("](") else { break };
        let alt = rest[..close].to_owned();
        let after = &rest[close + 2..];
        let Some(end) = after.find(')') else { break };
        let inside = after[..end].trim();
        // `<a b.png>` may hold spaces; else a title may follow one.
        let destination = match inside.strip_prefix('<').and_then(|r| r.split_once('>')) {
            Some((destination, _)) => destination,
            None => inside.split_once(' ').map_or(inside, |(d, _)| d),
        };
        rest = &after[end..];
        if ["kuria:", "http:", "https:", "data:"]
            .iter()
            .any(|p| destination.starts_with(p))
        {
            continue;
        }
        found.push((alt, destination.to_owned()));
    }
    found
}

/// A picture's media type by its file's extension, of those image akter
/// take; `None` for any other.
fn picture_type(file: &Path) -> Option<&'static str> {
    match file.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "svg" => Some("image/svg+xml"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// The pictures `markdown` embeds whose files are there, from `folder` if
/// relative, in a format an image akt takes.
fn pictures_in(markdown: &str, folder: &Path) -> Vec<Picture> {
    embedded(markdown)
        .into_iter()
        .filter_map(|(alt, destination)| {
            let path = Path::new(&destination);
            let file = if path.is_absolute() {
                path.to_owned()
            } else {
                folder.join(path)
            };
            (file.is_file() && picture_type(&file).is_some()).then_some(Picture {
                destination,
                alt,
                file,
            })
        })
        .collect()
}

/// How many pictures `markdown` embeds that aren't Kuria's.
fn pictures_left(markdown: &str) -> usize {
    embedded(markdown).len()
}

/// The akt type pictures are made as, and its title and picture fields.
#[derive(Clone, Debug)]
struct ImageType {
    key: String,
    title: String,
    picture: String,
}

/// The image type of those `documents` names, by key: `asked`, else the
/// one whose presentation names an `image` field; `None` if none does,
/// refused if several do and none is asked for (`cli.docs-import`).
fn image_type(
    documents: &[(String, Json)],
    asked: Option<&str>,
) -> Result<Option<ImageType>, CliError> {
    let of = |(key, document): &(String, Json)| -> Option<ImageType> {
        let picture = document["presentation"]["image"].as_str()?.to_owned();
        let fields = document["fields"].as_array()?;
        let title = document["presentation"]["title"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| {
                fields
                    .iter()
                    .find(|f| {
                        f["kind"] == "text" && f["required"] == true && f["multiline"] != true
                    })
                    .and_then(|f| f["key"].as_str().map(str::to_owned))
            })?;
        Some(ImageType {
            key: key.clone(),
            title,
            picture,
        })
    };
    let images: Vec<ImageType> = documents.iter().filter_map(of).collect();
    match asked {
        Some(key) => images.into_iter().find(|t| t.key == key).map(Some).ok_or_else(|| {
            CliError::refused(
                "BAD_REQUEST",
                format!("`{key}` isn't an akt type of pictures: its presentation names no `image` field"),
            )
        }),
        None if images.len() > 1 => Err(CliError::refused(
            "BAD_REQUEST",
            format!(
                "the draft's pictures can be made as {}: choose with --image-type",
                images.iter().map(|t| format!("`{}`", t.key)).collect::<Vec<_>>().join(" or ")
            ),
        )),
        None => Ok(images.into_iter().next()),
    }
}

/// The `--set`s that make an image akt of `ty` from `picture`, its file
/// given as `file`.
fn picture_sets(ty: &ImageType, picture: &Picture, file: &str) -> Vec<String> {
    let title = if picture.alt.trim().is_empty() {
        picture.file.file_stem().map_or_else(
            || "Picture".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        )
    } else {
        picture.alt.trim().to_owned()
    };
    vec![
        format!("{}={title}", ty.title),
        format!("{}={file}", ty.picture),
    ]
}

/// `markdown` with each picture `made` names embedded as its image akt.
fn embed(markdown: &str, made: &BTreeMap<String, String>) -> String {
    let mut text = markdown.to_owned();
    for (destination, number) in made {
        text = text.replace(&format!("]({destination})"), &format!("](kuria:{number})"));
    }
    text
}

/// The `--set` that makes `draft` the value of `shape`'s one `markdown`
/// field, written to a file of its own for `@<file>`; refused unless it
/// has exactly one.
fn draft_set(shape: &Shape, draft: &str) -> Result<(String, Scratch), CliError> {
    let field = markdown_field(shape)?;
    let name = getrandom::u64().expect("system randomness");
    let path = std::env::temp_dir().join(format!("kuria-draft-{name:016x}.md"));
    std::fs::write(&path, draft).map_err(|e| CliError::unavailable(e.to_string()))?;
    Ok((format!("{field}=@{}", path.display()), Scratch(path)))
}

/// `shape`'s one `markdown` field, which holds the draft.
fn markdown_field(shape: &Shape) -> Result<String, CliError> {
    let mut markdown = shape.fields.iter().filter(|f| f.kind == "MARKDOWN");
    match (markdown.next(), markdown.next()) {
        (Some(field), None) => Ok(field.key.clone()),
        _ => Err(bad(format!(
            "a draft needs `{}` to have exactly one markdown field to hold it",
            shape.key
        ))),
    }
}

/// A file or directory of the import's own, removed once done with.
#[derive(Debug)]
struct Scratch(PathBuf);

impl Scratch {
    fn dir() -> Result<Self, CliError> {
        let name = getrandom::u64().expect("system randomness");
        let path = std::env::temp_dir().join(format!("kuria-import-{name:016x}"));
        std::fs::create_dir_all(&path).map_err(|e| CliError::unavailable(e.to_string()))?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = if self.0.is_dir() {
            std::fs::remove_dir_all(&self.0)
        } else {
            std::fs::remove_file(&self.0)
        };
    }
}

/// What has been made so far, for a failure part way to say.
#[derive(Default)]
struct Made {
    number: Option<String>,
    versions: Vec<i64>,
    pictures: Vec<String>,
}

impl Made {
    fn said(&self, e: CliError) -> CliError {
        let Some(number) = &self.number else {
            return e;
        };
        let versions = match self.versions.as_slice() {
            [] => "no versions".to_owned(),
            [one] => format!("version {one}"),
            all => format!(
                "versions {}",
                all.iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        let pictures = if self.pictures.is_empty() {
            String::new()
        } else {
            format!(" and the pictures {}", self.pictures.join(", "))
        };
        CliError {
            message: format!(
                "{number} was made, with {versions}{pictures}; then {}",
                e.message
            ),
            ..e
        }
    }

    fn answer(&self, left: usize) -> Answer {
        let number = self.number.clone().unwrap_or_default();
        let count = self.versions.len();
        let mut lines = vec![format!(
            "Imported {number}, with {count} version{}.",
            if count == 1 { "" } else { "s" }
        )];
        if !self.pictures.is_empty() {
            lines.push(format!("Its pictures are {}.", self.pictures.join(", ")));
        }
        if left > 0 {
            lines.push(format!(
                "The draft's {left} other picture{} {} left as {}: add {} with Image.",
                if left == 1 { "" } else { "s" },
                if left == 1 { "was" } else { "were" },
                if left == 1 { "it was" } else { "they were" },
                if left == 1 { "it" } else { "them" }
            ));
        }
        Answer {
            markdown: lines.join("\n"),
            json: json!({
                "number": number,
                "versions": self.versions,
                "pictures": self.pictures,
                "picturesLeftOut": left,
            }),
        }
    }
}

/// What to import, and where.
pub struct Asked<'a> {
    pub folder: &'a Path,
    pub akt_type: &'a str,
    pub inside: &'a [String],
    pub image_type: Option<&'a str>,
    /// The pattern references are captured by, unless the folder's
    /// `metadata.json` gives its own.
    pub references: Option<&'a str>,
    /// Whether numbers written short are zero-padded to the pattern's
    /// widths (`--pad-numbers`).
    pub pad_numbers: bool,
    pub reason: Option<&'a str>,
}

/// The pictures of `draft`, by destination, each file once.
fn unique(draft: &Draft) -> Vec<&Picture> {
    let mut seen = std::collections::BTreeSet::new();
    draft
        .pictures
        .iter()
        .filter(|p| seen.insert(p.destination.clone()))
        .collect()
}

/// `kuria docs import` against a server.
// r[impl cli.docs-import]
pub async fn on_server(session: &Session, asked: &Asked<'_>) -> Result<Answer, CliError> {
    let shape = crate::reads::shape(crate::reads::Source::Server(session), asked.akt_type).await?;
    let padding = asked
        .pad_numbers
        .then(|| Padding::of(&shape, asked.inside))
        .transpose()?;
    let folder = read(asked.folder, asked.references, padding.as_ref())?;
    if folder.draft.is_some() {
        markdown_field(&shape)?;
    }
    let pictured = folder
        .draft
        .as_ref()
        .is_some_and(|d| !d.pictures.is_empty());
    let images = if pictured {
        let types = session
            .graphql("{ aktTypes { key document } }", json!({}))
            .await?;
        let documents: Vec<(String, Json)> = types["aktTypes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| {
                (
                    t["key"].as_str().unwrap_or_default().to_owned(),
                    t["document"].clone(),
                )
            })
            .collect();
        image_type(&documents, asked.image_type)?
    } else {
        None
    };
    let mut made = Made::default();
    let created = crate::writes::create(
        session,
        &crate::writes::Created {
            akt_type: asked.akt_type,
            inside: asked.inside,
            sets: &folder.sets,
            file: None,
            number: folder.number.as_deref(),
            reason: asked.reason,
        },
    )
    .await?;
    let number = created.json["number"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    made.number = Some(number.clone());
    for version in &folder.versions {
        let released =
            crate::versions::release(session, std::slice::from_ref(&number), &releasing(version))
                .await
                .map_err(|e| made.said(e))?;
        made.versions
            .push(released.json["version"].as_i64().unwrap_or_default());
    }
    let mut left = 0;
    if let Some(draft) = &folder.draft {
        let mut numbers = BTreeMap::new();
        if let Some(ty) = &images {
            for picture in unique(draft) {
                let media = picture_type(&picture.file).unwrap_or("application/octet-stream");
                let id = crate::versions::upload(session, &picture.file, media)
                    .await
                    .map_err(|e| made.said(e))?;
                let created = crate::writes::create(
                    session,
                    &crate::writes::Created {
                        akt_type: &ty.key,
                        inside: &[],
                        sets: &picture_sets(ty, picture, &id),
                        file: None,
                        number: None,
                        reason: asked.reason,
                    },
                )
                .await
                .map_err(|e| made.said(e))?;
                let image = created.json["number"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                made.pictures.push(image.clone());
                numbers.insert(picture.destination.clone(), image);
            }
        }
        let markdown = embed(&draft.markdown, &numbers);
        left = pictures_left(&markdown);
        let (set, _file) = draft_set(&shape, &markdown)?;
        crate::writes::set(session, &number, std::slice::from_ref(&set), asked.reason)
            .await
            .map_err(|e| made.said(e))?;
    }
    Ok(made.answer(left))
}

fn releasing(version: &Version) -> Releasing<'_> {
    Releasing {
        label: version.label.as_deref(),
        description: version.description.as_deref(),
        file: Some(&version.file),
    }
}

/// `kuria docs import` in an akt directory: each step committed, since a
/// version is read from git (`cli.local.versions`).
// r[impl cli.docs-import]
pub fn in_dir(root: &Path, me: Option<String>, asked: &Asked<'_>) -> Result<Answer, CliError> {
    use crate::local::{edits, git, versions, write::Writer};
    committed(root)?;
    let mut w = Writer::open(root, me.clone())?;
    let shape = w.shape(asked.akt_type)?;
    let padding = asked
        .pad_numbers
        .then(|| Padding::of(&shape, asked.inside))
        .transpose()?;
    let folder = read(asked.folder, asked.references, padding.as_ref())?;
    if folder.draft.is_some() {
        markdown_field(&shape)?;
    }
    let images = if folder
        .draft
        .as_ref()
        .is_some_and(|d| !d.pictures.is_empty())
    {
        let documents: Vec<(String, Json)> = w
            .dir
            .types
            .iter()
            .map(|(key, t)| {
                (
                    key.clone(),
                    serde_json::to_value(t.ty.document()).unwrap_or_default(),
                )
            })
            .collect();
        image_type(&documents, asked.image_type)?
    } else {
        None
    };
    let mut made = Made::default();
    let created = edits::create(
        &mut w,
        &crate::writes::Created {
            akt_type: asked.akt_type,
            inside: asked.inside,
            sets: &folder.sets,
            file: None,
            number: folder.number.as_deref(),
            reason: asked.reason,
        },
    )?;
    w.save()?;
    let number = created.json["number"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    made.number = Some(number.clone());
    git::commit(
        root,
        &format!("Import {number} from {}", asked.folder.display()),
    )
    .map_err(|e| made.said(e))?;
    for version in &folder.versions {
        let times = git::Times::of(root);
        let mut w = Writer::open(root, me.clone()).map_err(|e| made.said(e))?;
        let released = versions::release(
            &mut w,
            &times,
            std::slice::from_ref(&number),
            &releasing(version),
        )
        .map_err(|e| made.said(e))?;
        w.save().map_err(|e| made.said(e))?;
        let n = released.json["version"].as_i64().unwrap_or_default();
        git::commit(root, &format!("Import {number}: version {n}")).map_err(|e| made.said(e))?;
        made.versions.push(n);
    }
    let left = match &folder.draft {
        Some(draft) => {
            let mut w = Writer::open(root, me).map_err(|e| made.said(e))?;
            let left = draft_in(
                &mut w,
                &number,
                draft,
                &shape,
                images.as_ref(),
                asked,
                &mut made,
            )
            .map_err(|e| made.said(e))?;
            w.save().map_err(|e| made.said(e))?;
            git::commit(root, &format!("Import {number}: its draft")).map_err(|e| made.said(e))?;
            left
        }
        None => 0,
    };
    Ok(made.answer(left))
}

/// Refuses an akt directory with changes git hasn't committed, or none at
/// all, since an import commits each step.
fn committed(root: &Path) -> Result<(), CliError> {
    match crate::local::git::uncommitted(root) {
        Some(false) => Ok(()),
        Some(true) => Err(CliError::refused(
            "UNCOMMITTED",
            "the directory has changes git hasn't committed: commit them, then import, which commits each step",
        )),
        None => Err(CliError::refused(
            "BAD_REQUEST",
            "an import commits each step, so the akt directory must be in a git repository",
        )),
    }
}

/// Makes `draft`'s pictures image akter of `images` and the draft, so
/// embedding them, the body of `number`, in local mode; answers how many
/// pictures it left as they were.
fn draft_in(
    w: &mut crate::local::write::Writer,
    number: &str,
    draft: &Draft,
    shape: &Shape,
    images: Option<&ImageType>,
    asked: &Asked<'_>,
    made: &mut Made,
) -> Result<usize, CliError> {
    use crate::local::edits;
    let mut numbers = BTreeMap::new();
    if let Some(ty) = images {
        for picture in unique(draft) {
            let file = format!("@{}", picture.file.display());
            let sets = picture_sets(ty, picture, &file);
            let created = edits::create(
                w,
                &crate::writes::Created {
                    akt_type: &ty.key,
                    inside: &[],
                    sets: &sets,
                    file: None,
                    number: None,
                    reason: asked.reason,
                },
            )?;
            let image = created.json["number"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            made.pictures.push(image.clone());
            numbers.insert(picture.destination.clone(), image);
        }
    }
    let markdown = embed(&draft.markdown, &numbers);
    let (set, _file) = draft_set(shape, &markdown)?;
    edits::set(w, number, std::slice::from_ref(&set), asked.reason)?;
    Ok(pictures_left(&markdown))
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cli.docs-import]
    #[test]
    fn finds_the_pictures_a_draft_embeds() {
        let markdown = "![a](media/image1.png) and ![b](kuria:IMG-1) ![c](<x y.png> \"T\") ![d](https://e/f.png)";
        assert_eq!(
            embedded(markdown),
            [
                ("a".to_owned(), "media/image1.png".to_owned()),
                ("c".to_owned(), "x y.png".to_owned())
            ]
        );
        assert_eq!(pictures_left("no pictures [link](x)"), 0);
        let made = BTreeMap::from([("media/image1.png".to_owned(), "IMG-4".to_owned())]);
        assert_eq!(embed("![a](media/image1.png)", &made), "![a](kuria:IMG-4)");
    }

    // r[verify cli.docs-import]
    #[test]
    fn writes_metadata_values_as_sets() {
        assert_eq!(set_of("title", &json!("My doc")).unwrap(), "title=My doc");
        assert_eq!(set_of("tags", &json!(["a", "b"])).unwrap(), "tags=a,b");
        assert_eq!(set_of("n", &json!(3)).unwrap(), "n=3");
        assert!(set_of("x", &json!({})).is_err());
    }

    // r[verify cli.docs-import]
    #[test]
    fn finds_the_one_image_type_or_asks_which() {
        let image = |key: &str| {
            (
                key.to_owned(),
                json!({ "presentation": { "image": "file" }, "fields": [
                    { "key": "title", "kind": "text", "required": true },
                    { "key": "file", "kind": "attachment" }
                ] }),
            )
        };
        let plain = (
            "note".to_owned(),
            json!({ "presentation": {}, "fields": [] }),
        );
        let one = image_type(&[image("image"), plain.clone()], None)
            .unwrap()
            .unwrap();
        assert_eq!(
            (one.key.as_str(), one.title.as_str(), one.picture.as_str()),
            ("image", "title", "file")
        );
        assert!(
            image_type(std::slice::from_ref(&plain), None)
                .unwrap()
                .is_none()
        );
        assert!(image_type(&[image("image"), image("photo")], None).is_err());
        assert_eq!(
            image_type(&[image("image"), image("photo")], Some("photo"))
                .unwrap()
                .unwrap()
                .key,
            "photo"
        );
        assert!(image_type(&[plain], Some("note")).is_err());
    }

    // r[verify cli.docs-import]
    #[test]
    fn captures_references_outside_code_and_links() {
        let pattern = references_of(r"(?<id>FOOID-\d{4})(?<label>(?:\s?\p{Lu}\w+)*)").unwrap();
        let draft = "See FOOID-1234 Supplier Policy for details, and FOOID-0042.\n\
                     Not `FOOID-9999` in code, nor [FOOID-1111](kuria:FOOID-1111).\n\
                     ```\nFOOID-8888 Code Block\n```\n";
        assert_eq!(
            capture(draft, &pattern, None),
            "See [Supplier Policy](kuria:FOOID-1234) for details, and [FOOID-0042](kuria:FOOID-0042).\n\
             Not `FOOID-9999` in code, nor [FOOID-1111](kuria:FOOID-1111).\n\
             ```\nFOOID-8888 Code Block\n```\n"
        );
        assert!(references_of(r"FOOID-\d{4}").is_err());
        assert!(references_of(r"(?<id>").is_err());
    }
}
