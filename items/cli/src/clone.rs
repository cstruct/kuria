//! Clones (`clone.*`): a container and its members as files on someone's
//! machine, synced with Kuria through `kuria`.
//!
//! - [`layout`]: where each file goes and what it holds (`clone.tree`),
//!   and a file in normal form (`clone.normal`).
//! - [`pictures`]: the pictures of the images the files embed
//!   (`clone.pictures`).
//! - [`fmt`]: `kuria docs fmt`, a clone's files put in normal form.
//! - [`base`]: what a clone keeps in `.kuria/`: its base and what is
//!   staged (`clone.clone`).
//! - [`work`]: `kuria docs status`, `diff` and `add`, offline (`clone.status`,
//!   `clone.add`).
//! - [`sync`]: `kuria docs clone`, `pull` and `push` (`clone.clone`,
//!   `clone.pull`, `clone.push`).
//!
//! [`CloneOf`] reads what a clone holds from Kuria.

pub mod base;
pub mod fmt;
pub mod layout;
pub mod pictures;
pub mod sync;
pub mod work;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde_json::{Map, Value as Json, json};

use crate::error::CliError;
use crate::model::{SHAPE, SOMEONE, Shape, VALUES};
use crate::session::Session;

use base::LiveBase;
use layout::{AT_REVISION, filed, read_file, title_in, tree_of};

/// Where a clone keeps what it knows of Kuria, beside its files.
pub const KURIA_DIR: &str = ".kuria";

/// A clone of the container `number`, at `root`, on the Kuria `session`
/// speaks to.
pub struct CloneOf<'s> {
    pub session: &'s Session,
    pub number: String,
    pub root: PathBuf,
}

/// The picture an akt shows: its attachment and media type, if its type
/// names one and it holds one.
const PICTURE: &str = "number type { presentation { image } } fields { ... on AttachmentValue { field attachment { id mediaType } } }";

/// The picture an akt shows, from what `akt { PICTURE }` answered.
fn picture_in(akt: &Json) -> Option<(String, String)> {
    let field = akt["type"]["presentation"]["image"].as_str()?;
    akt["fields"].as_array()?.iter().find_map(|f| {
        (f["field"] == field).then(|| {
            Some((
                f["attachment"]["id"].as_str()?.to_owned(),
                f["attachment"]["mediaType"].as_str()?.to_owned(),
            ))
        })?
    })
}

/// How many akter one request asks for at most: few enough to stay within
/// the server's complexity limit, which splits a request that doesn't.
const AKTER_PER_REQUEST: usize = 25;
const FILES_PER_REQUEST: usize = 10;

/// Whether Kuria refused a request for being too big.
fn too_big(e: &CliError) -> bool {
    let message = e.message.to_lowercase();
    message.contains("too complex") || message.contains("nested too deep")
}

/// What `akt { … }` selects for [`CloneOf::now`], with the `Shape`, `Values`
/// and `Someone` fragments defined.
const NOW: &str = "number title revision state { key } type { ...Shape } fields { ...Values } liveState { state base }";

/// An akt as Kuria has it now, as a clone's base records it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AktNow {
    /// Where its file goes.
    pub path: String,
    pub revision: i64,
    /// Its live draft, if it has one.
    pub live: Option<LiveBase>,
}

/// A container and its members as Kuria has them now, as a clone holds
/// them (`clone.tree`, `clone.pictures`).
#[derive(Clone, Debug, Default)]
pub struct Now {
    /// Each file, by path, its images linked to their pictures.
    pub files: BTreeMap<String, String>,
    /// Each picture, by path.
    pub pictures: BTreeMap<String, Vec<u8>>,
    /// Each akt, by number.
    pub akter: BTreeMap<String, AktNow>,
}

impl CloneOf<'_> {
    /// The container's title, refused unless it is a container the subject
    /// may read (`clone.clone`).
    // r[impl clone.clone]
    pub async fn container(&self) -> Result<Option<String>, CliError> {
        let data = self
            .session
            .graphql(
                "query($n: String!) { akt(number: $n) { title type { name document } } }",
                json!({ "n": self.number }),
            )
            .await?;
        let akt = &data["akt"];
        if akt.is_null() {
            return Err(CliError::from_server(
                "NOT_FOUND",
                format!(
                    "No akt `{}`, or none you may read: a clone is of a container you may read",
                    self.number
                ),
                None,
            ));
        }
        let contains = akt["type"]["document"]["contains"]
            .as_array()
            .is_some_and(|c| !c.is_empty());
        if !contains {
            return Err(CliError::refused(
                "NOT_A_CONTAINER",
                format!(
                    "{} is a {}, which contains no akter: a clone is of a container and its members",
                    self.number,
                    akt["type"]["name"].as_str().unwrap_or("akt")
                ),
            ));
        }
        Ok(akt["title"].as_str().map(str::to_owned))
    }

    /// The container and its members as Kuria has them now: each file where
    /// the container's `sections` puts it, its images linked to their
    /// pictures, each picture, and each akt's revision and live draft
    /// (`clone.tree`, `clone.pictures`, `live.state`). A member the subject
    /// may not read is left out.
    // r[impl clone.tree]
    pub async fn now(&self) -> Result<Now, CliError> {
        self.container().await?;
        let members =
            crate::reads::members_of(crate::reads::Source::Server(self.session), &self.number)
                .await?;
        let mut asked = vec![(self.number.clone(), None)];
        asked.extend(members.into_iter().map(|(n, _)| (n, None)));
        let fragments = format!("{SHAPE} {VALUES} {SOMEONE}");
        let found = self
            .found(&asked, NOW, &fragments, FILES_PER_REQUEST)
            .await?;
        let mut container = None;
        let mut texts = Vec::new();
        let mut akter = BTreeMap::new();
        for ((number, _), akt) in asked.iter().zip(&found) {
            let Some(akt) = akt else { continue };
            let file = filed(number, akt)?;
            let live = akt["liveState"]["state"].as_str().map(|state| LiveBase {
                state: state.to_owned(),
                base: akt["liveState"]["base"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            });
            akter.insert(
                number.clone(),
                AktNow {
                    path: String::new(),
                    revision: akt["revision"].as_i64().unwrap_or_default(),
                    live,
                },
            );
            if *number == self.number {
                container = Some(file);
            } else {
                texts.push((number.clone(), file.text, file.title));
            }
        }
        let Some(container) = container else {
            return Err(CliError::from_server(
                "NOT_FOUND",
                format!("No akt `{}`", self.number),
                None,
            ));
        };
        let tree = tree_of(
            &self.number,
            &container.text,
            container.title.as_deref(),
            container.sections.as_ref(),
            &texts,
        );
        let (links, pictures) = self.pictures_now(&tree).await?;
        let mut files = BTreeMap::new();
        for (path, text) in tree {
            if let Some(number) = layout::number_of_file(&text)
                && let Some(akt) = akter.get_mut(&number)
            {
                akt.path.clone_from(&path);
            }
            let linked = pictures::linked_relatively(&text, &path, &links);
            files.insert(path, linked);
        }
        Ok(Now {
            files,
            pictures,
            akter,
        })
    }

    /// Each of `asked`, as [`CloneOf::found`] answers it, refusing one that
    /// isn't there as `NOT_FOUND`.
    async fn akter(
        &self,
        asked: &[(String, Option<i64>)],
        body: &str,
        fragments: &str,
        per_request: usize,
    ) -> Result<Vec<Json>, CliError> {
        self.found(asked, body, fragments, per_request)
            .await?
            .into_iter()
            .zip(asked)
            .map(|(akt, (number, _))| {
                akt.ok_or_else(|| {
                    CliError::from_server("NOT_FOUND", format!("No akt `{number}`"), None)
                })
            })
            .collect()
    }

    /// Each of `asked`, an akt by number, at a revision if one is given,
    /// with `body` selected and `fragments` defined: several to a request,
    /// splitting one Kuria finds too big. Answers them in order, `None` for
    /// one that isn't there or the subject may not read.
    pub async fn found(
        &self,
        asked: &[(String, Option<i64>)],
        body: &str,
        fragments: &str,
        per_request: usize,
    ) -> Result<Vec<Option<Json>>, CliError> {
        let mut answers = Vec::with_capacity(asked.len());
        let mut pending: Vec<&[(String, Option<i64>)]> =
            asked.chunks(per_request.max(1)).rev().collect();
        while let Some(chunk) = pending.pop() {
            let mut declared = Vec::new();
            let mut selected = Vec::new();
            let mut variables = Map::new();
            for (i, (number, revision)) in chunk.iter().enumerate() {
                declared.push(format!("$n{i}: String!"));
                variables.insert(format!("n{i}"), json!(number));
                let at = match revision {
                    Some(r) => {
                        declared.push(format!("$r{i}: Int"));
                        variables.insert(format!("r{i}"), json!(r));
                        format!(", revision: $r{i}")
                    }
                    None => String::new(),
                };
                selected.push(format!("a{i}: akt(number: $n{i}{at}) {{ {body} }}"));
            }
            let query = format!(
                "query({}) {{ {} }} {fragments}",
                declared.join(", "),
                selected.join(" ")
            );
            match self.session.graphql(&query, Json::Object(variables)).await {
                Ok(data) => {
                    for i in 0..chunk.len() {
                        let akt = &data[format!("a{i}")];
                        answers.push((!akt.is_null()).then(|| akt.clone()));
                    }
                }
                Err(e) if chunk.len() > 1 && too_big(&e) => {
                    let (first, second) = chunk.split_at(chunk.len() / 2);
                    pending.push(second);
                    pending.push(first);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(answers)
    }

    /// Each of `asked`, an akt by number, as Kuria has it now or at a
    /// revision, as its file in a clone says it, with its images linked to
    /// Kuria (`clone.tree`).
    pub async fn files_of(&self, asked: &[(String, Option<i64>)]) -> Result<Vec<String>, CliError> {
        let fragments = format!("{SHAPE} {VALUES} {SOMEONE}");
        let answers = self
            .akter(asked, AT_REVISION, &fragments, FILES_PER_REQUEST)
            .await?;
        asked
            .iter()
            .zip(&answers)
            .map(|((number, _), akt)| filed(number, akt).map(|f| f.text))
            .collect()
    }

    /// The pictures of the images the files of `kept`, with their images
    /// linked to Kuria, embed, as Kuria has them now: where each image's
    /// picture goes, by number, and each picture's path, to its bytes. An
    /// image that is one of the clone's own files is a file, not a picture
    /// (`clone.pictures`).
    // r[impl clone.pictures]
    async fn pictures_now(
        &self,
        kept: &BTreeMap<String, String>,
    ) -> Result<(HashMap<String, String>, BTreeMap<String, Vec<u8>>), CliError> {
        let files: Vec<String> = kept
            .values()
            .filter_map(|t| read_file(t).ok().map(|(n, _)| n))
            .collect();
        let mut numbers: Vec<String> = Vec::new();
        for text in kept.values() {
            for number in pictures::embedded(text) {
                if !files.contains(&number) && !numbers.contains(&number) {
                    numbers.push(number);
                }
            }
        }
        let asked: Vec<(String, Option<i64>)> = numbers.iter().map(|n| (n.clone(), None)).collect();
        let found = self.found(&asked, PICTURE, "", AKTER_PER_REQUEST).await?;
        let mut links = HashMap::new();
        let mut pictures = BTreeMap::new();
        for (number, akt) in numbers.iter().zip(found) {
            if let Some((id, media_type)) = akt.as_ref().and_then(picture_in) {
                let path = pictures::path_of(number, &media_type);
                pictures.insert(path.clone(), self.picture_bytes(&id).await?);
                links.insert(number.clone(), path);
            }
        }
        Ok((links, pictures))
    }

    /// The bytes of the attachment `id`: from the clone's `.kuria/` if it
    /// kept them, else from Kuria, keeping them there if the clone has one.
    async fn picture_bytes(&self, id: &str) -> Result<Vec<u8>, CliError> {
        let kuria = self.root.join(KURIA_DIR);
        let path = kuria.join("pictures").join(id);
        if let Ok(bytes) = std::fs::read(&path) {
            return Ok(bytes);
        }
        let url = format!("{}/attachments/{id}", self.session.url);
        let response = self
            .session
            .http
            .send(
                reqwest::Method::GET,
                &url,
                Some(self.session.access_token()),
                &crate::http::Body::Empty,
            )
            .await?;
        if !response.status().is_success() {
            return Err(CliError::unavailable(format!(
                "Kuria answered {} for the picture {id}",
                response.status()
            )));
        }
        let bytes = response.bytes().await?.to_vec();
        if kuria.is_dir()
            && let Some(dir) = path.parent()
            && std::fs::create_dir_all(dir).is_ok()
        {
            let _ = std::fs::write(&path, &bytes);
        }
        Ok(bytes)
    }

    async fn shape(
        &self,
        key: &str,
        shapes: &mut HashMap<String, Shape>,
    ) -> Result<Shape, CliError> {
        if let Some(shape) = shapes.get(key) {
            return Ok(shape.clone());
        }
        let shape = crate::reads::shape(crate::reads::Source::Server(self.session), key).await?;
        shapes.insert(key.to_owned(), shape.clone());
        Ok(shape)
    }

    /// The files as Kuria would write them, from `tree`'s files: each in
    /// normal form, where the container's `sections` puts it, its embedded
    /// images linked to their pictures; and those pictures, by path
    /// (`clone.normal`, `clone.tree`, `clone.pictures`).
    // r[impl clone.normal]
    // r[impl clone.tree]
    pub async fn normal_tree(
        &self,
        tree: &BTreeMap<String, String>,
    ) -> Result<(BTreeMap<String, String>, BTreeMap<String, Vec<u8>>), CliError> {
        let kept = pictures::kept(tree);
        let (links, pictures) = self.pictures_now(&kept).await?;
        let mut shapes = HashMap::new();
        let mut container = None;
        let mut members = Vec::new();
        for text in kept.values() {
            let (number, parsed) = read_file(text)?;
            let key = parsed.text("type").unwrap_or_default();
            let shape = self.shape(&key, &mut shapes).await?;
            let normal = layout::normal(&shape, text)?;
            let title = title_in(&shape, &parsed);
            if number == self.number {
                let sections = shape
                    .fields
                    .iter()
                    .find(|f| f.kind == "SECTIONS")
                    .and_then(|f| parsed.values().ok()?.get(&f.key).cloned())
                    .and_then(|v| v.as_str().map(crate::values::sections_from_text))
                    .and_then(Result::ok);
                container = Some((normal, title, sections));
            } else {
                members.push((number, normal, title));
            }
        }
        let Some((text, title, sections)) = container else {
            return Err(CliError::refused(
                "BAD_FILE",
                format!("the clone has no file of {}", self.number),
            ));
        };
        let texts = tree_of(
            &self.number,
            &text,
            title.as_deref(),
            sections.as_ref(),
            &members,
        )
        .into_iter()
        .map(|(path, text)| {
            let linked = pictures::linked_relatively(&text, &path, &links);
            (path, linked)
        })
        .collect();
        Ok((texts, pictures))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_a_request_kuria_finds_too_big() {
        let refused = |m: &str| CliError::from_server("BAD_REQUEST", m.to_owned(), None);
        assert!(too_big(&refused("Query is too complex.")));
        assert!(too_big(&refused("Query is nested too deep.")));
        assert!(!too_big(&refused("No akt `D-9`")));
    }
}
