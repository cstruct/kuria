//! `cat`, `show` and `history`: one akt at a time.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::file::{self, AKT, Akt, Other};
use crate::history::{Described, REVISIONS, Revision, describe};
use crate::model::{SHAPE, SOMEONE, VALUES, read};
use crate::output::Answer;
use crate::profile::io_error;
use crate::session::Session;
use crate::values::text_of;

/// How many members `show` lists.
const MEMBERS: u32 = 200;

async fn akt_data(session: &Session, number: &str, extra: &str) -> Result<Json, CliError> {
    let data = session
        .graphql(
            &format!("query($n: String!) {{ akt(number: $n) {{ {AKT} {extra} }} }} {SHAPE} {VALUES} {SOMEONE}"),
            json!({ "n": number }),
        )
        .await?;
    if data["akt"].is_null() {
        return Err(CliError::from_server(
            "NOT_FOUND",
            format!("No akt `{number}`"),
            None,
        ));
    }
    Ok(data["akt"].clone())
}

/// `kuria cat <number>…`: each akt as its file, or written to `output` as
/// `<number>.md`.
// r[impl cli.read]
// r[impl cli.file]
pub async fn cat(
    session: &Session,
    numbers: &[String],
    output: Option<&Path>,
) -> Result<Answer, CliError> {
    let mut akter = Vec::new();
    for number in numbers {
        let akt: Akt = read(akt_data(session, number, "").await?, "the akt")?;
        akter.push(akt);
    }
    let files: Vec<String> = akter.iter().map(file::write).collect();
    let json = json!({ "akter": akter.iter().map(file::json).collect::<Vec<_>>() });
    let Some(dir) = output else {
        return Ok(Answer {
            markdown: files.join("\n"),
            json,
        });
    };
    std::fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
    let mut written = Vec::new();
    for (akt, text) in akter.iter().zip(&files) {
        let path = dir.join(format!("{}.md", akt.number));
        std::fs::write(&path, text).map_err(|e| io_error(&path, &e))?;
        written.push(path.display().to_string());
    }
    Ok(Answer {
        markdown: written
            .iter()
            .map(|p| format!("Wrote {p}"))
            .collect::<Vec<_>>()
            .join("\n"),
        json: json!({ "written": written }),
    })
}

#[derive(Deserialize)]
struct Around {
    board: Board,
    members: Members,
    #[serde(default)]
    revisions: Vec<Revision>,
}

#[derive(Deserialize)]
struct Board {
    columns: Vec<Column>,
}

#[derive(Deserialize)]
struct Column {
    key: String,
    name: String,
    akter: Vec<Other>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Members {
    nodes: Vec<Other>,
    page_info: HasNext,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HasNext {
    has_next_page: bool,
}

fn other(o: &Other) -> String {
    let title = o
        .title
        .as_deref()
        .map(|t| format!(" · {t}"))
        .unwrap_or_default();
    format!(
        "{}{title} ({}, {})",
        o.number, o.akt_type.name, o.state.name
    )
}

fn other_json(o: &Other) -> Json {
    json!({ "number": o.number, "title": o.title, "type": o.akt_type.key, "state": o.state.key })
}

/// `kuria show <number>`: its fields, containers, members, links and the
/// transitions its state allows, and with `history` its revisions.
// r[impl cli.read]
pub async fn show(session: &Session, number: &str, with_history: bool) -> Result<Answer, CliError> {
    let extra = format!(
        "board(first: {MEMBERS}) {{ columns {{ key name akter {{ number title type {{ key name }} state {{ key name }} }} }} }}
         members(first: {MEMBERS}) {{ nodes {{ number title type {{ key name }} state {{ key name }} }} pageInfo {{ hasNextPage }} }}
         {}",
        if with_history { REVISIONS } else { "" }
    );
    let data = akt_data(session, number, &extra).await?;
    let akt: Akt = read(data.clone(), "the akt")?;
    let around: Around = read(data, "the akt")?;
    let shape = &akt.shape;
    let has_board = shape.document["board"].is_object();

    let columns: Vec<&Column> = around
        .board
        .columns
        .iter()
        .filter(|c| has_board && !(c.key == "unmapped" && c.akter.is_empty()))
        .collect();
    let allowed: Vec<_> = shape
        .transitions
        .iter()
        .filter(|t| t.from.contains(&akt.state.key))
        .collect();
    let described = described(&around.revisions, shape);
    let parts = Shown {
        akt: &akt,
        around: &around,
        columns: &columns,
        allowed: &allowed,
        described: &described,
        with_history,
    };
    Ok(Answer {
        markdown: parts.markdown(),
        json: json!({ "akt": parts.json() }),
    })
}

/// What `show` says, gathered.
struct Shown<'a> {
    akt: &'a Akt,
    around: &'a Around,
    columns: &'a [&'a Column],
    allowed: &'a [&'a crate::model::Transition],
    described: &'a [Described],
    with_history: bool,
}

impl Shown<'_> {
    fn markdown(&self) -> String {
        let Self {
            akt,
            allowed,
            described,
            with_history,
            ..
        } = *self;
        let shape = &akt.shape;
        let mut text = format!("# {}", akt.number);
        if let Some(title) = &akt.title {
            text.push_str(&format!(" · {title}"));
        }
        text.push_str(&format!(
            "\n\n{} · {} · revision {} · changed {}\n",
            shape.name,
            akt.state.name,
            akt.revision,
            crate::history::when(&akt.updated_at)
        ));

        let title_field = shape.presentation.title.as_deref();
        let (sections, inline): (Vec<_>, Vec<_>) = akt
            .fields
            .iter()
            .filter(|v| Some(v.field()) != title_field)
            .partition(|v| shape.field(v.field()).is_some_and(|f| f.multiline));
        if !inline.is_empty() || !sections.is_empty() {
            text.push_str("\n## Fields\n\n");
            for value in &inline {
                text.push_str(&format!(
                    "- **{}:** {}\n",
                    shape.field_name(value.field()),
                    text_of(value)
                ));
            }
            for value in &sections {
                text.push_str(&format!(
                    "\n### {}\n\n{}\n",
                    shape.field_name(value.field()),
                    text_of(value).trim_end()
                ));
            }
        }
        if !akt.containers.is_empty() {
            text.push_str("\n## In\n\n");
            for c in &akt.containers {
                text.push_str(&format!("- {}\n", other(c)));
            }
        }
        text.push_str(&self.members());
        if !akt.links.is_empty() {
            text.push_str("\n## Links\n\n");
            for link in &akt.links {
                let mut line = format!("- {} {}", link.name, other(&link.akt));
                if let Some(pinned) = &link.pinned {
                    line.push_str(&format!(
                        ", pinned at revision {}: {}, {}",
                        pinned.revision,
                        pinned.title.as_deref().unwrap_or(&link.akt.number),
                        pinned.state.name
                    ));
                }
                text.push_str(&line);
                text.push('\n');
            }
        }
        if !allowed.is_empty() {
            text.push_str("\n## Transitions\n\n");
            for t in allowed {
                let to = shape
                    .state(&t.to)
                    .map_or(t.to.as_str(), |s| s.name.as_str());
                let mut line = format!("- `{}` {} → {to}", t.key, t.name);
                if !t.requires.is_empty() {
                    let names: Vec<&str> = t.requires.iter().map(|k| shape.field_name(k)).collect();
                    line.push_str(&format!(", requires {}", names.join(", ")));
                }
                text.push_str(&line);
                text.push('\n');
            }
        }
        if with_history {
            text.push_str("\n## History\n\n");
            for d in described {
                text.push_str(&d.markdown());
            }
        }

        text
    }

    /// Its members: by column if its type has a board, else by type.
    fn members(&self) -> String {
        let Self {
            akt,
            around,
            columns,
            ..
        } = *self;
        let mut text = String::new();
        if !columns.is_empty() {
            text.push_str("\n## Members\n");
            for column in columns {
                text.push_str(&format!("\n### {}\n\n", column.name));
                if column.akter.is_empty() {
                    text.push_str("None.\n");
                }
                for member in &column.akter {
                    text.push_str(&format!("- {}\n", other(member)));
                }
            }
        } else if !around.members.nodes.is_empty() {
            text.push_str("\n## Members\n");
            let mut by_type: Vec<(&str, Vec<&Other>)> = Vec::new();
            for member in &around.members.nodes {
                match by_type.iter_mut().find(|(t, _)| *t == member.akt_type.name) {
                    Some((_, members)) => members.push(member),
                    None => by_type.push((&member.akt_type.name, vec![member])),
                }
            }
            for (type_name, members) in &by_type {
                text.push_str(&format!("\n### {type_name}\n\n"));
                for member in members {
                    text.push_str(&format!("- {}\n", other(member)));
                }
            }
        }
        if around.members.page_info.has_next_page {
            text.push_str(&format!("\nMore members: `kuria ls --in {}`\n", akt.number));
        }
        text
    }

    fn json(&self) -> Json {
        let Self {
            akt,
            around,
            columns,
            allowed,
            described,
            with_history,
        } = *self;
        let shape = &akt.shape;
        let mut json = file::json(akt);
        json["title"] = akt.title.clone().into();
        json["typeName"] = shape.name.clone().into();
        json["stateName"] = akt.state.name.clone().into();
        json["createdAt"] = akt.created_at.clone().into();
        json["updatedAt"] = akt.updated_at.clone().into();
        json["containers"] = akt.containers.iter().map(other_json).collect();
        json["links"] = akt
            .links
            .iter()
            .map(|l| {
                json!({
                    "kind": l.kind, "name": l.name, "outgoing": l.outgoing,
                    "akt": other_json(&l.akt), "pinnedRevision": l.pinned_revision,
                })
            })
            .collect();
        json["members"] = if columns.is_empty() {
            json!({ "akter": around.members.nodes.iter().map(other_json).collect::<Vec<_>>(), "more": around.members.page_info.has_next_page })
        } else {
            json!({ "columns": columns.iter().map(|c| json!({
                "key": c.key, "name": c.name, "akter": c.akter.iter().map(other_json).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(), "more": around.members.page_info.has_next_page })
        };
        json["transitions"] = allowed
            .iter()
            .map(|t| json!({ "key": t.key, "name": t.name, "to": t.to, "requires": t.requires }))
            .collect();
        if with_history {
            json["history"] = described.iter().map(Described::json).collect();
        }
        json
    }
}

fn described(revisions: &[Revision], shape: &crate::model::Shape) -> Vec<Described> {
    let fields: HashMap<String, String> = shape
        .fields
        .iter()
        .map(|f| (f.key.clone(), f.name.clone()))
        .collect();
    let transitions: HashMap<String, String> = shape
        .transitions
        .iter()
        .map(|t| (t.key.clone(), t.name.clone()))
        .collect();
    revisions
        .iter()
        .map(|r| describe(r, &fields, &transitions))
        .collect()
}

/// `kuria history <number>`: its revisions, as the app's history says them.
// r[impl cli.read]
pub async fn history(session: &Session, number: &str) -> Result<Answer, CliError> {
    let data = session
        .graphql(
            &format!("query($n: String!) {{ akt(number: $n) {{ number type {{ ...Shape }} {REVISIONS} }} }} {SHAPE}"),
            json!({ "n": number }),
        )
        .await?;
    if data["akt"].is_null() {
        return Err(CliError::from_server(
            "NOT_FOUND",
            format!("No akt `{number}`"),
            None,
        ));
    }
    let shape: crate::model::Shape = read(data["akt"]["type"].clone(), "the akt type")?;
    let revisions: Vec<Revision> = read(data["akt"]["revisions"].clone(), "the revisions")?;
    let described = described(&revisions, &shape);
    let mut markdown = format!("# {number} · History\n\n");
    for d in &described {
        markdown.push_str(&d.markdown());
    }
    Ok(Answer {
        markdown,
        json: json!({ "number": number, "revisions": described.iter().map(Described::json).collect::<Vec<_>>() }),
    })
}
