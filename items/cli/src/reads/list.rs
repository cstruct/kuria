//! `ls` and `find`: akter as a table, a page at a time.

use serde_json::{Map, Value as Json, json};

use super::{Local, Source};
use crate::error::CliError;
use crate::local::answers;
use crate::md;
use crate::model::{Page, ROW, Row, SOMEONE, Shape, VALUES, read};
use crate::output::Answer;
use crate::values::{People, condition_value, file_value, text_of};

/// The most akter one page may hold.
const MOST: u32 = 200;

/// What `ls` was asked for.
#[derive(Clone, Debug, Default)]
pub struct Listing {
    /// The akt type's key.
    pub akt_type: Option<String>,
    /// A state, by key or name.
    pub state: Option<String>,
    /// A container's number, to list its members.
    pub inside: Option<String>,
    /// `<field>=<value>` conditions, all of which an akt must meet.
    pub conditions: Vec<String>,
    pub search: Option<String>,
    pub limit: u32,
    /// The cursor a previous page ended at.
    pub after: Option<String>,
    /// Every page, not one.
    pub all: bool,
}

/// `kuria ls`.
// r[impl cli.read]
pub async fn ls(source: Source<'_>, listing: &Listing) -> Result<Answer, CliError> {
    let shape = match &listing.akt_type {
        Some(key) => Some(super::shape(source, key).await?),
        None => None,
    };
    let state = match (&listing.state, &shape) {
        (Some(state), Some(shape)) => Some(state_key(shape, state)?),
        (Some(state), None) => Some(state.clone()),
        _ => None,
    };
    let conditions = conditions(source, shape.as_ref(), &listing.conditions).await?;
    let mut rows = Vec::new();
    let mut after = listing.after.clone();
    let first = if listing.all {
        MOST
    } else {
        listing.limit.clamp(1, MOST)
    };
    let page_info = loop {
        let page = page(
            source,
            listing,
            state.as_deref(),
            &conditions,
            first,
            after.as_deref(),
        )
        .await?;
        rows.extend(page.nodes);
        if !(listing.all && page.page_info.has_next_page) {
            break page.page_info;
        }
        after = page.page_info.end_cursor;
    };
    // Members have no state to ask for: the page is filtered here.
    if listing.inside.is_some()
        && let Some(state) = &state
    {
        rows.retain(|r| r.state.key.as_deref() == Some(state.as_str()));
    }
    Ok(answer(&rows, shape.as_ref(), &page_info, listing.all))
}

/// `kuria find <text>`: akter by a number's beginning or a title's words.
// r[impl cli.read]
pub async fn find(source: Source<'_>, text: &str, limit: u32) -> Result<Answer, CliError> {
    ls(
        source,
        &Listing {
            search: Some(text.to_owned()),
            limit,
            ..Listing::default()
        },
    )
    .await
}

fn state_key(shape: &Shape, state: &str) -> Result<String, CliError> {
    shape
        .states
        .iter()
        .find(|s| s.key == state)
        .or_else(|| {
            shape
                .states
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(state))
        })
        .map(|s| s.key.clone())
        .ok_or_else(|| {
            CliError::refused(
                "UNKNOWN_STATE",
                format!(
                    "{} has no state `{state}`: {}",
                    shape.name,
                    shape
                        .states
                        .iter()
                        .map(|s| format!("`{}`", s.key))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        })
}

/// `--where` as `FieldCondition`s.
async fn conditions(
    source: Source<'_>,
    shape: Option<&Shape>,
    given: &[String],
) -> Result<Vec<Json>, CliError> {
    if given.is_empty() {
        return Ok(Vec::new());
    }
    let Some(shape) = shape else {
        return Err(CliError::refused(
            "BAD_REQUEST",
            "--where filters one akt type's akter: name the type, as `kuria ls <type> --where …`",
        ));
    };
    let mut people: Option<People> = None;
    let mut conditions = Vec::new();
    for condition in given {
        let Some((key, value)) = condition.split_once('=') else {
            return Err(CliError::refused(
                "BAD_REQUEST",
                format!("`{condition}` isn't `<field>=<value>`"),
            ));
        };
        let key = key.trim();
        let field = shape.field(key).ok_or_else(|| CliError {
            field: Some(key.to_owned()),
            ..CliError::refused(
                "UNKNOWN_FIELD",
                format!("{} has no field `{key}`", shape.name),
            )
        })?;
        let person = matches!(field.kind.as_str(), "USER" | "CHECKBOX");
        let equals = match source {
            // r[impl cli.local.people]
            Source::Local(local) if person => {
                let who = value.trim().strip_prefix("for:").unwrap_or(value.trim());
                if who == "@me" {
                    local.me()?.to_owned()
                } else {
                    who.to_owned()
                }
            }
            _ => {
                if person && people.is_none() {
                    people = Some(super::people(source).await?);
                }
                condition_value(field, value, people.as_ref().unwrap_or(&People::default()))?
            }
        };
        conditions.push(json!({ "field": key, "equals": equals }));
    }
    Ok(conditions)
}

async fn page(
    source: Source<'_>,
    listing: &Listing,
    state: Option<&str>,
    conditions: &[Json],
    first: u32,
    after: Option<&str>,
) -> Result<Page, CliError> {
    let session = match source {
        Source::Server(session) => session,
        Source::Local(local) => return local_page(local, listing, state, conditions, first, after),
    };
    let fragments = format!("{ROW} {VALUES} {SOMEONE}");
    let wheres = (!conditions.is_empty()).then(|| conditions.to_vec());
    if let Some(container) = &listing.inside {
        let data = session
            .graphql(
                &format!(
                    "query($n: String!, $type: String, $first: Int, $after: String, $where: [FieldCondition!]) {{
                      akt(number: $n) {{ members(type: $type, first: $first, after: $after, where: $where) {{
                        nodes {{ ...Row }} pageInfo {{ endCursor hasNextPage }} }} }} }} {fragments}"
                ),
                json!({ "n": container, "type": listing.akt_type, "first": first, "after": after, "where": wheres }),
            )
            .await?;
        if data["akt"].is_null() {
            return Err(CliError::from_server(
                "NOT_FOUND",
                format!("No akt `{container}`"),
                None,
            ));
        }
        return read(data["akt"]["members"].clone(), "the members");
    }
    let data = session
        .graphql(
            &format!(
                "query($type: String, $state: String, $first: Int, $after: String, $search: String, $where: [FieldCondition!]) {{
                  akter(type: $type, state: $state, first: $first, after: $after, search: $search, where: $where) {{
                    nodes {{ ...Row }} pageInfo {{ endCursor hasNextPage }} }} }} {fragments}"
            ),
            json!({
                "type": listing.akt_type, "state": state, "first": first, "after": after,
                "search": listing.search, "where": wheres,
            }),
        )
        .await?;
    read(data["akter"].clone(), "the akter")
}

/// Whether the akt file value `value` meets `equals` for a field of `kind`.
fn meets(kind: &str, value: &Json, equals: &str) -> bool {
    match kind {
        "CHOICES" => value
            .as_array()
            .is_some_and(|keys| keys.iter().any(|k| k == equals)),
        "USER" => value
            .as_str()
            .is_some_and(|v| v.eq_ignore_ascii_case(equals)),
        "CHECKBOX" => value
            .as_str()
            .and_then(|v| v.strip_prefix("for:"))
            .is_some_and(|who| who.eq_ignore_ascii_case(equals)),
        _ => value.as_str() == Some(equals),
    }
}

/// Whether `akt` is what `search` asks: a number starting with it, or a
/// title holding each of its words, ignoring case.
fn found(number: &str, title: Option<&str>, search: &str) -> bool {
    let search = search.to_lowercase();
    let title = title.unwrap_or_default().to_lowercase();
    number.to_lowercase().starts_with(&search)
        || search.split_whitespace().all(|word| title.contains(word))
}

/// A page of the directory's akter: a container's members in its order,
/// else every akt, most recently changed first.
// r[impl cli.local.layout]
fn local_page(
    local: &Local,
    listing: &Listing,
    state: Option<&str>,
    conditions: &[Json],
    first: u32,
    after: Option<&str>,
) -> Result<Page, CliError> {
    let dir = &local.dir;
    let mut akter: Vec<_> = match &listing.inside {
        Some(container) => {
            if dir.akt(container).is_none() {
                return Err(CliError::from_server(
                    "NOT_FOUND",
                    format!("No akt `{container}`"),
                    None,
                ));
            }
            dir.members_of(container).collect()
        }
        None => {
            let mut all: Vec<_> = dir.akter.iter().collect();
            let updated = |a: &&crate::local::dir::LocalAkt| local.times.of_akt(a).1;
            all.sort_by(|a, b| {
                updated(b)
                    .cmp(&updated(a))
                    .then_with(|| b.number.cmp(&a.number))
            });
            all
        }
    };
    akter.retain(|a| {
        let ty = &dir.type_of(a).ty;
        listing.akt_type.as_ref().is_none_or(|t| *t == a.type_key)
            && state.is_none_or(|s| s == a.state)
            && listing
                .search
                .as_ref()
                .is_none_or(|s| found(&a.number, answers::title(dir, a).as_deref(), s))
            && conditions.iter().all(|c| {
                let key = c["field"].as_str().unwrap_or_default();
                let kind = ty
                    .field(key)
                    .map(crate::local::answers::kind_of)
                    .unwrap_or_default();
                a.values
                    .get(key)
                    .is_some_and(|v| meets(&kind, v, c["equals"].as_str().unwrap_or_default()))
            })
    });
    let start = match after {
        None => 0,
        Some(cursor) => cursor
            .strip_prefix("local:")
            .and_then(|n| n.parse::<usize>().ok())
            .ok_or_else(|| {
                CliError::refused("BAD_REQUEST", "that cursor isn't one of this list's")
            })?,
    };
    let end = (start + first as usize).min(akter.len());
    let nodes = akter
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .map(|a| answers::row(dir, &local.times, a))
        .collect::<Result<Vec<_>, _>>()?;
    let has_next = end < akter.len();
    read(
        json!({
            "nodes": nodes,
            "pageInfo": { "endCursor": (end > start).then(|| format!("local:{end}")), "hasNextPage": has_next },
        }),
        "the akter",
    )
}

fn answer(rows: &[Row], shape: Option<&Shape>, page: &crate::model::PageInfo, all: bool) -> Answer {
    let list: Vec<&str> = shape
        .map(|s| s.presentation.list.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let mut headers = vec!["Number"];
    if shape.is_none() {
        headers.push("Type");
    }
    headers.extend(["Title", "State"]);
    // r[impl cli.read]
    let outcomes = rows.iter().any(|r| r.outcome.is_some());
    if outcomes {
        headers.push("Outcome");
    }
    let names: Vec<&str> = list
        .iter()
        .map(|k| shape.map_or(*k, |s| s.field_name(k)))
        .collect();
    headers.extend(&names);
    let table: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            let mut cells = vec![r.number.clone()];
            if shape.is_none() {
                cells.push(r.akt_type.name.clone());
            }
            cells.push(r.title.clone().unwrap_or_default());
            cells.push(r.state.name.clone());
            if outcomes {
                cells.push(
                    r.outcome
                        .as_deref()
                        .map(str::to_lowercase)
                        .unwrap_or_default(),
                );
            }
            for key in &list {
                cells.push(
                    r.fields
                        .iter()
                        .find(|v| v.field() == *key)
                        .map(text_of)
                        .unwrap_or_default(),
                );
            }
            cells
        })
        .collect();
    let mut markdown = if rows.is_empty() {
        "No akter.".to_owned()
    } else {
        md::table(&headers, &table)
    };
    if page.has_next_page
        && !all
        && let Some(cursor) = &page.end_cursor
    {
        markdown.push_str(&format!(
            "\nMore follow: add `--after {cursor}`, or `--all` for every page.\n"
        ));
    }
    Answer {
        markdown,
        json: json!({
            "akter": rows.iter().map(row_json).collect::<Vec<_>>(),
            "endCursor": if all { None } else { page.end_cursor.clone() },
            "hasNextPage": page.has_next_page && !all,
        }),
    }
}

fn row_json(row: &Row) -> Json {
    let fields: Map<String, Json> = row
        .fields
        .iter()
        .map(|v| (v.field().to_owned(), file_value(v)))
        .collect();
    json!({
        "number": row.number,
        "type": row.akt_type.key,
        "title": row.title,
        "state": row.state.key,
        "outcome": row.outcome.as_ref().map(|o| o.to_lowercase()),
        "updatedAt": row.updated_at,
        "fields": fields,
    })
}
