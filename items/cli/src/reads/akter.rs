//! `cat`, `show` and `history`: one akt at a time.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value as Json, json};

use super::{Local, Source};
use crate::error::CliError;
use crate::file::{self, AKT, Akt, Other};
use crate::history::{Described, REVISIONS, Revision, describe};
use crate::local::answers;
use kuria_engine::outcomes::Outcome;
use kuria_engine::sections::Item;

use crate::model::{ARCHIVED, SHAPE, SOMEONE, VALUES, Value, read};
use crate::output::Answer;
use crate::profile::io_error;
use crate::values::text_of;

/// How many members `show` lists.
const MEMBERS: u32 = 200;

/// The akt `number`, at its version `version` if given (`version.read`).
///
/// In an akt directory, its members and board answer those `members`
/// allows; against a server, `extra` asks for them.
pub(crate) async fn akt_data(
    source: Source<'_>,
    number: &str,
    extra: &str,
    version: Option<i64>,
    members: super::Archived,
) -> Result<Json, CliError> {
    let missing = || match version {
        Some(v) => CliError::from_server("NOT_FOUND", format!("{number} has no version {v}"), None),
        None => not_found(number),
    };
    let session = match source {
        Source::Server(session) => session,
        Source::Local(l) => {
            let found = match version {
                Some(v) => answers::akt_at(&l.dir, &l.times, number, v)?,
                None => answers::akt(&l.dir, &l.times, number, members)?,
            };
            return found.ok_or_else(missing);
        }
    };
    let data = session
        .graphql(
            &format!("query($n: String!, $v: Int) {{ akt(number: $n, version: $v) {{ {AKT} {extra} }} }} {SHAPE} {VALUES} {SOMEONE}"),
            json!({ "n": number, "v": version }),
        )
        .await?;
    if data["akt"].is_null() {
        return Err(missing());
    }
    Ok(data["akt"].clone())
}

fn not_found(number: &str) -> CliError {
    CliError::from_server("NOT_FOUND", format!("No akt `{number}`"), None)
}

/// Which members `cat` gives after each akt (`cli.members`).
#[derive(Clone, Copy, Debug)]
pub enum WithMembers<'a> {
    None,
    Types(&'a [String]),
    All,
}

/// The direct members of `container` that `archived` allows, in its
/// order, with their types' keys.
// r[impl cli.members]
// r[impl cli.archive]
pub(crate) async fn members_of(
    source: Source<'_>,
    container: &str,
    archived: super::Archived,
) -> Result<Vec<(String, String)>, CliError> {
    let session = match source {
        Source::Server(session) => session,
        Source::Local(l) => {
            return Ok(l
                .dir
                .members_with(container, archived)
                .map(|m| (m.number.clone(), m.type_key.clone()))
                .collect());
        }
    };
    let mut found = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let data = session
            .graphql(
                &format!(
                    "query($n: String!, $after: String, $archived: ArchivedFilter) {{ akt(number: $n) {{
                       members(first: {MEMBERS}, after: $after, archived: $archived) {{
                         nodes {{ number type {{ key }} }} pageInfo {{ hasNextPage endCursor }} }} }} }}"
                ),
                json!({ "n": container, "after": after, "archived": archived.filter() }),
            )
            .await?;
        let members = &data["akt"]["members"];
        for node in members["nodes"].as_array().into_iter().flatten() {
            if let (Some(n), Some(k)) = (node["number"].as_str(), node["type"]["key"].as_str()) {
                found.push((n.to_owned(), k.to_owned()));
            }
        }
        if members["pageInfo"]["hasNextPage"] != true {
            return Ok(found);
        }
        after = members["pageInfo"]["endCursor"].as_str().map(str::to_owned);
    }
}

/// `kuria cat <number>…`: each akt as its file, followed by the members
/// `with` asks for that `archived` allows, several as one multi-akt file,
/// or each written to `output` as `<number>.md`.
// r[impl cli.read]
// r[impl cli.file]
// r[impl cli.members]
pub async fn cat(
    source: Source<'_>,
    numbers: &[String],
    output: Option<&Path>,
    with: WithMembers<'_>,
    version: Option<i64>,
    archived: super::Archived,
) -> Result<Answer, CliError> {
    if version.is_some() && (numbers.len() > 1 || !matches!(with, WithMembers::None)) {
        return Err(CliError::refused(
            "BAD_REQUEST",
            "`--version` reads one akt, without its members",
        ));
    }
    if let WithMembers::Types(keys) = with {
        for key in keys {
            super::shape(source, key).await?;
        }
    }
    let mut wanted: Vec<String> = Vec::new();
    for number in numbers {
        // Known before its members are asked for, so a missing one says so.
        let akt: Akt = read(
            akt_data(source, number, "", version, archived).await?,
            "the akt",
        )?;
        wanted.push(akt.number);
        let members = match with {
            WithMembers::None => Vec::new(),
            _ => members_of(source, number, archived).await?,
        };
        for (member, key) in members {
            let asked = match with {
                WithMembers::Types(keys) => keys.contains(&key),
                _ => true,
            };
            if asked {
                wanted.push(member);
            }
        }
    }
    let mut akter = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for number in wanted {
        if seen.insert(number.clone()) {
            akter.push(read::<Akt>(
                akt_data(source, &number, "", version, archived).await?,
                "the akt",
            )?);
        }
    }
    let files: Vec<String> = akter.iter().map(file::write).collect();
    let json = json!({ "akter": akter.iter().map(file::json).collect::<Vec<_>>() });
    let Some(dir) = output else {
        return Ok(Answer {
            markdown: file::write_all(&akter),
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
    /// What the transitions its state allows wait for
    /// (`akt.transition.waits`).
    #[serde(default)]
    waits: Vec<Wait>,
}

#[derive(Deserialize)]
struct Wait {
    transition: String,
    open: Vec<String>,
    lacking: Vec<String>,
}

impl Wait {
    /// What it waits for, in words: "TASK-1 and TASK-2, and a Verified by link".
    fn text(&self) -> String {
        let all = |words: &[String]| match words {
            [] => String::new(),
            [one] => one.clone(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        };
        let mut parts = Vec::new();
        if !self.open.is_empty() {
            parts.push(all(&self.open));
        }
        if !self.lacking.is_empty() {
            let links: Vec<String> = self.lacking.iter().map(|k| format!("a {k} link")).collect();
            parts.push(all(&links));
        }
        parts.join(", and ")
    }
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
    let outcome = o
        .outcome
        .as_deref()
        .map(|o| format!(" — {}", o.to_lowercase()))
        .unwrap_or_default();
    // r[impl cli.archive]
    let archived = if o.archived.is_some() {
        ", archived"
    } else {
        ""
    };
    format!(
        "{}{title} ({}, {}{archived}){outcome}",
        o.number, o.akt_type.name, o.state.name
    )
}

fn other_json(o: &Other) -> Json {
    let mut json = json!({ "number": o.number, "title": o.title, "type": o.akt_type.key, "state": o.state.key });
    if let Some(outcome) = &o.outcome {
        json["outcome"] = outcome.to_lowercase().into();
    }
    if let Some(archived) = &o.archived {
        json["archived"] = archived.json();
    }
    json
}

/// The akt's sections, if it has them.
fn sections_of(akt: &Akt) -> Option<Vec<Item>> {
    akt.fields.iter().find_map(|v| match v {
        Value::SectionsValue { value, .. } => kuria_engine::sections::from_json(value).ok(),
        _ => None,
    })
}

/// An outcome as GraphQL names it, read.
fn outcome_of(text: &str) -> Option<Outcome> {
    match text.to_ascii_lowercase().as_str() {
        "pass" => Some(Outcome::Pass),
        "fail" => Some(Outcome::Fail),
        "skip" => Some(Outcome::Skip),
        "pending" => Some(Outcome::Pending),
        _ => None,
    }
}

/// Sections' items as `show` says them: a nested list, each section with
/// its title, outcome (from its members', `member_outcome`) and text, each
/// member a line.
// r[impl cli.read]
// r[impl akt.outcome.section]
fn sections_markdown(
    items: &[Item],
    depth: usize,
    member: &dyn Fn(&str) -> String,
    member_outcome: &dyn Fn(&str) -> Option<Outcome>,
) -> String {
    let indent = "  ".repeat(depth);
    let mut text = String::new();
    for item in items {
        match item {
            Item::Akt(number) => text.push_str(&format!("{indent}- {}\n", member(number))),
            Item::Section(section) => {
                let title = section.title.as_deref().unwrap_or("Untitled section");
                let outcome = kuria_engine::sections::outcome(section, member_outcome);
                text.push_str(&format!("{indent}- **{title}** — {}\n", outcome.as_str()));
                if let Some(said) = &section.text {
                    for line in said.trim_end().lines() {
                        if line.trim().is_empty() {
                            text.push('\n');
                        } else {
                            text.push_str(&format!("{indent}  {line}\n"));
                        }
                    }
                }
                text.push_str(&sections_markdown(
                    &section.items,
                    depth + 1,
                    member,
                    member_outcome,
                ));
            }
        }
    }
    text
}

/// Sections' items in JSON, with each section's outcome.
fn sections_json(items: &[Item], member_outcome: &dyn Fn(&str) -> Option<Outcome>) -> Json {
    items
        .iter()
        .map(|item| match item {
            Item::Akt(number) => json!(number),
            Item::Section(s) => json!({
                "title": s.title,
                "text": s.text,
                "collapsible": s.collapsible,
                "outcome": kuria_engine::sections::outcome(s, member_outcome).as_str(),
                "items": sections_json(&s.items, member_outcome),
            }),
        })
        .collect()
}

/// `kuria show <number>`: its fields, containers, members, links and the
/// transitions its state allows, and with `history` its revisions.
// r[impl cli.read]
// r[impl cli.archive]
pub async fn show(
    source: Source<'_>,
    number: &str,
    with_history: bool,
    version: Option<i64>,
    archived: super::Archived,
) -> Result<Answer, CliError> {
    // A live container's board shows its live members, an archived one's
    // all of them (`archive.board`), unless asked otherwise.
    let board = match archived {
        super::Archived::Live => String::new(),
        asked => format!(", archived: {}", asked.filter()),
    };
    let extra = format!(
        "board(first: {MEMBERS}{board}) {{ columns {{ key name akter {{ number title type {{ key name }} state {{ key name }} outcome {ARCHIVED} }} }} }}
         members(first: {MEMBERS}, archived: {}) {{ nodes {{ number title type {{ key name }} state {{ key name }} outcome {ARCHIVED} }} pageInfo {{ hasNextPage }} }}
         waits {{ transition open lacking }}
         {}",
        // Kept below as the board keeps them: all of an archived one's.
        match archived {
            super::Archived::Live => "ALL",
            asked => asked.filter(),
        },
        if with_history { REVISIONS } else { "" }
    );
    let data = akt_data(source, number, &extra, version, archived).await?;
    let akt: Akt = read(data.clone(), "the akt")?;
    let mut around: Around = read(data, "the akt")?;
    if archived == super::Archived::Live && akt.archived.is_none() {
        around.members.nodes.retain(|m| m.archived.is_none());
    }
    let shape = &akt.shape;
    let has_board = shape.document["board"].is_object();

    // r[impl app.members]
    // The board's columns, and the members it doesn't map, listed by type,
    // as the app shows them.
    let columns: Vec<&Column> = around
        .board
        .columns
        .iter()
        .filter(|c| has_board && c.key != "unmapped")
        .collect();
    let listed: Vec<&Other> = if has_board {
        around
            .board
            .columns
            .iter()
            .filter(|c| c.key == "unmapped")
            .flat_map(|c| c.akter.iter())
            .collect()
    } else {
        around.members.nodes.iter().collect()
    };
    let allowed: Vec<_> = shape
        .transitions
        .iter()
        .filter(|t| t.from.contains(&akt.state.key))
        .collect();
    let described = match source {
        Source::Local(l) if with_history => local_history(l, number)?.0,
        _ => described(&around.revisions, shape),
    };
    let parts = Shown {
        akt: &akt,
        around: &around,
        columns: &columns,
        listed: &listed,
        allowed: &allowed,
        described: &described,
        with_history,
    };
    Ok(Answer {
        markdown: parts.markdown(),
        json: json!({ "akt": parts.json() }),
    })
}

/// How `show` begins: the akt's number and title, then its type, state,
/// outcome, revision, when it changed and whether it is archived.
fn heading(akt: &Akt) -> String {
    let mut text = format!("# {}", akt.number);
    if let Some(title) = &akt.title {
        text.push_str(&format!(" · {title}"));
    }
    let outcome = akt
        .outcome
        .as_deref()
        .map(|o| format!(" · {}", o.to_lowercase()))
        .unwrap_or_default();
    // r[impl cli.archive]
    let archived = akt
        .archived
        .as_ref()
        .map(|a| format!(" · {}", a.said()))
        .unwrap_or_default();
    text.push_str(&format!(
        "\n\n{} · {}{outcome} · revision {} · changed {}{archived}\n{}",
        akt.shape.name,
        akt.state.name,
        akt.revision,
        crate::history::when(&akt.updated_at),
        at_version(akt)
    ));
    text
}

/// What `show` says, gathered.
struct Shown<'a> {
    akt: &'a Akt,
    around: &'a Around,
    columns: &'a [&'a Column],
    /// The members no column holds, listed by type.
    listed: &'a [&'a Other],
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
        let mut text = heading(akt);

        let title_field = shape.presentation.title.as_deref();
        let (sections, inline): (Vec<_>, Vec<_>) = akt
            .fields
            .iter()
            .filter(|v| Some(v.field()) != title_field)
            .filter(|v| !matches!(v, Value::SectionsValue { .. }))
            .partition(|v| {
                shape
                    .field(v.field())
                    .is_some_and(crate::model::FieldDef::sectioned)
            });
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
        text.push_str(&broken_references(akt));
        text.push_str(&versions_of(akt));
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
                // r[impl akt.transition.waits]
                if let Some(wait) = self.around.waits.iter().find(|w| w.transition == t.key) {
                    line.push_str(&format!(", waits for {}", wait.text()));
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
            listed,
            ..
        } = *self;
        let mut text = String::new();
        if let Some(items) = sections_of(akt) {
            return self.in_sections(&items);
        }
        if !columns.is_empty() || !listed.is_empty() {
            text.push_str("\n## Members\n");
        }
        for column in columns {
            text.push_str(&format!("\n### {}\n\n", column.name));
            if column.akter.is_empty() {
                text.push_str("None.\n");
            }
            for member in &column.akter {
                text.push_str(&format!("- {}\n", other(member)));
            }
        }
        let mut by_type: Vec<(&str, Vec<&Other>)> = Vec::new();
        for member in listed {
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
        if around.members.page_info.has_next_page {
            text.push_str(&format!("\nMore members: `kuria ls --in {}`\n", akt.number));
        }
        text
    }

    /// Its members' outcomes, by number.
    fn member_outcomes(&self) -> HashMap<&str, Outcome> {
        self.around
            .members
            .nodes
            .iter()
            .filter_map(|m| Some((m.number.as_str(), outcome_of(m.outcome.as_deref()?)?)))
            .collect()
    }

    /// Its members in its sections, then those the sections don't place,
    /// by type.
    fn in_sections(&self, items: &[Item]) -> String {
        let Self { akt, around, .. } = *self;
        let by_number: HashMap<&str, &Other> = around
            .members
            .nodes
            .iter()
            .map(|m| (m.number.as_str(), m))
            .collect();
        let member = |n: &str| by_number.get(n).map_or_else(|| n.to_owned(), |m| other(m));
        let outcomes = self.member_outcomes();
        let member_outcome = |n: &str| outcomes.get(n).copied();
        let mut text = String::from("\n## Sections\n\n");
        text.push_str(&sections_markdown(items, 0, &member, &member_outcome));
        let placed = kuria_engine::sections::numbers(items);
        let mut by_type: Vec<(&str, Vec<&Other>)> = Vec::new();
        for m in around
            .members
            .nodes
            .iter()
            .filter(|m| !placed.contains(&m.number.as_str()))
        {
            match by_type.iter_mut().find(|(t, _)| *t == m.akt_type.name) {
                Some((_, members)) => members.push(m),
                None => by_type.push((&m.akt_type.name, vec![m])),
            }
        }
        if !by_type.is_empty() {
            text.push_str("\n## Other members\n");
        }
        for (type_name, members) in &by_type {
            text.push_str(&format!("\n### {type_name}\n\n"));
            for m in members {
                text.push_str(&format!("- {}\n", other(m)));
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
            listed,
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
                    "byReference": l.by_reference,
                })
            })
            .collect();
        json["version"] = akt.version.as_ref().map(|v| v.number).into();
        json["versions"] = akt
            .versions
            .iter()
            .map(|v| json!({ "number": v.number, "revision": v.revision, "label": v.label }))
            .collect();
        json["references"] = references(akt)
            .map(|(field, r)| json!({ "field": field, "destination": r.destination, "broken": r.broken }))
            .collect();
        let mut members = json!({
            "akter": listed.iter().map(|o| other_json(o)).collect::<Vec<_>>(),
            "more": around.members.page_info.has_next_page,
        });
        if !columns.is_empty() {
            members["columns"] = columns
                .iter()
                .map(|c| json!({ "key": c.key, "name": c.name, "akter": c.akter.iter().map(other_json).collect::<Vec<_>>() }))
                .collect();
        }
        if let Some(items) = sections_of(akt) {
            let outcomes = self.member_outcomes();
            members["sections"] = sections_json(&items, &|n| outcomes.get(n).copied());
        }
        json["members"] = members;
        json["transitions"] = allowed
            .iter()
            .map(|t| {
                let wait = self.around.waits.iter().find(|w| w.transition == t.key);
                json!({
                    "key": t.key, "name": t.name, "to": t.to, "requires": t.requires,
                    "waits": wait.map(|w| json!({ "open": w.open, "lacking": w.lacking })),
                })
            })
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

/// The commits that changed an akt, as revisions are said, and whether it
/// has changes not committed yet; `None` outside git.
// r[impl cli.local.history]
fn local_history(local: &Local, number: &str) -> Result<(Vec<Described>, Option<bool>), CliError> {
    let akt = local.dir.any(number).ok_or_else(|| not_found(number))?;
    let Some((commits, dirty)) = local.times.history(akt) else {
        return Ok((Vec::new(), None));
    };
    let described = commits
        .into_iter()
        .enumerate()
        .map(|(i, (c, revision))| Described {
            revision: revision.into(),
            who: c.author,
            at: c.at,
            what: if i == 0 { "created it" } else { "changed it" }.to_owned(),
            reason: Some(c.subject),
            changes: Vec::new(),
            memberships: Vec::new(),
        })
        .collect();
    Ok((described, Some(dirty)))
}

/// `kuria history <number>`: its revisions, as the app's history says them;
/// in local mode, the commits that changed its file.
// r[impl cli.read]
pub async fn history(source: Source<'_>, number: &str) -> Result<Answer, CliError> {
    let session = match source {
        Source::Server(session) => session,
        Source::Local(l) => {
            let (mut described, dirty) = local_history(l, number)?;
            if let Some(akt) = l.dir.akt(number) {
                let versions: Vec<(i64, i64, Option<String>)> = akt
                    .versions
                    .iter()
                    .map(|v| {
                        (
                            v["revision"].as_i64().unwrap_or_default(),
                            v["number"].as_i64().unwrap_or_default(),
                            v["label"].as_str().map(str::to_owned),
                        )
                    })
                    .collect();
                released(&mut described, &versions);
            }
            let mut markdown = format!("# {number} · History\n\n");
            for d in &described {
                markdown.push_str(&d.markdown());
            }
            match dirty {
                None => {
                    markdown.push_str("No history: the akt directory isn't in a git repository.\n");
                }
                Some(true) => markdown.push_str("\nIt has changes not committed yet.\n"),
                Some(false) if described.is_empty() => markdown.push_str("No commits yet.\n"),
                Some(false) => {}
            }
            return Ok(Answer {
                markdown,
                json: json!({
                    "number": number,
                    "revisions": described.iter().map(Described::json).collect::<Vec<_>>(),
                    "tracked": dirty.is_some(),
                    "uncommitted": dirty.unwrap_or(false),
                }),
            });
        }
    };
    let data = session
        .graphql(
            &format!("query($n: String!) {{ akt(number: $n) {{ number type {{ ...Shape }} versions {{ number revision label }} {REVISIONS} }} }} {SHAPE}"),
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
    let mut described = described(&revisions, &shape);
    let versions: Vec<file::Released> =
        read(data["akt"]["versions"].clone(), "the versions").unwrap_or_default();
    let versions: Vec<(i64, i64, Option<String>)> = versions
        .into_iter()
        .map(|v| (v.revision, v.number, v.label))
        .collect();
    released(&mut described, &versions);
    let mut markdown = format!("# {number} · History\n\n");
    for d in &described {
        markdown.push_str(&d.markdown());
    }
    Ok(Answer {
        markdown,
        json: json!({ "number": number, "revisions": described.iter().map(Described::json).collect::<Vec<_>>() }),
    })
}

/// The references in an akt's `markdown` fields, each with its field.
// r[impl akt.field.markdown.references]
fn references(akt: &file::Akt) -> impl Iterator<Item = (&str, &crate::model::MarkdownReference)> {
    akt.fields.iter().flat_map(|v| match v {
        Value::MarkdownValue {
            field, references, ..
        } => references
            .iter()
            .map(|r| (field.as_str(), r))
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    })
}

/// An akt's broken references, as `show` says them, if it has any.
fn broken_references(akt: &file::Akt) -> String {
    let broken: Vec<String> = references(akt)
        .filter_map(|(field, r)| {
            let why = r.broken.as_ref()?;
            Some(format!(
                "- {}: `{}`: {why}\n",
                akt.shape.field_name(field),
                r.destination
            ))
        })
        .collect();
    if broken.is_empty() {
        String::new()
    } else {
        format!("\n## Broken references\n\n{}", broken.concat())
    }
}

/// That the akt is read at a version, if it is (`version.read`).
fn at_version(akt: &file::Akt) -> String {
    akt.version.as_ref().map_or_else(String::new, |v| {
        format!(
            "\nAt version {}{}: not the living version.\n",
            v.number,
            v.label
                .as_deref()
                .map(|l| format!(" ({l})"))
                .unwrap_or_default()
        )
    })
}

/// An akt's versions, latest first, as `show` lists them.
// r[impl version.list]
fn versions_of(akt: &file::Akt) -> String {
    if akt.versions.is_empty() {
        return String::new();
    }
    let mut text = String::from("\n## Versions\n\n");
    for v in &akt.versions {
        text.push_str(&format!(
            "- {}{} · revision {}{}\n",
            v.number,
            v.label
                .as_deref()
                .map(|l| format!(" ({l})"))
                .unwrap_or_default(),
            v.revision,
            v.released_at
                .as_deref()
                .map(|at| format!(" · released {}", crate::history::when(at)))
                .unwrap_or_default()
        ));
    }
    text
}

/// Says, of each revision a version names, that it was released
/// (`version.history`): `versions` are `(revision, number, label)`.
// r[impl version.history]
fn released(described: &mut [Described], versions: &[(i64, i64, Option<String>)]) {
    for d in described.iter_mut() {
        for (revision, number, label) in versions {
            if *revision == d.revision {
                d.what.push_str(&format!(
                    ", released as version {number}{}",
                    label
                        .as_deref()
                        .map(|l| format!(" ({l})"))
                        .unwrap_or_default()
                ));
            }
        }
    }
}
