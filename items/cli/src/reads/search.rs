//! `kuria search`: akter by the words their text fields hold
//! (`akt.search.text`), then those `find` finds by number or title
//! (`mcp.search`).

use kuria_config::FieldKind;
use serde::Deserialize;
use serde_json::{Value as Json, json};

use super::{Archived, Local, Source};
use crate::error::CliError;
use crate::local::answers;
use crate::md;
use crate::output::Answer;

/// What `kuria search` was asked for.
#[derive(Clone, Debug)]
pub struct Searching<'a> {
    pub text: &'a str,
    /// Only this akt type's akter.
    pub akt_type: Option<&'a str>,
    pub limit: u32,
    pub archived: Archived,
}

/// One field of one akt that the text matched.
#[derive(Clone, Debug, PartialEq)]
struct Match {
    number: String,
    title: Option<String>,
    type_key: String,
    field: String,
    excerpt: String,
}

#[derive(Deserialize)]
struct Found {
    akt: FoundAkt,
    field: String,
    excerpt: String,
}

#[derive(Deserialize)]
struct FoundAkt {
    number: String,
    title: Option<String>,
    #[serde(rename = "type")]
    akt_type: FoundType,
}

#[derive(Deserialize)]
struct FoundType {
    key: String,
}

/// `kuria search <text>`.
// r[impl cli.read]
// r[impl mcp.search]
pub async fn search(source: Source<'_>, asked: &Searching<'_>) -> Result<Answer, CliError> {
    let limit = asked.limit.clamp(1, 50);
    let matches = match source {
        Source::Server(session) => {
            let data = session
                .graphql(
                    "query($t: String!, $type: String, $first: Int, $archived: ArchivedFilter) {
                       textSearch(text: $t, type: $type, first: $first, archived: $archived) {
                         akt { number title type { key } } field excerpt } }",
                    json!({ "t": asked.text, "type": asked.akt_type, "first": limit, "archived": asked.archived.filter() }),
                )
                .await?;
            let found: Vec<Found> = serde_json::from_value(data["textSearch"].clone())
                .map_err(|e| CliError::unavailable(format!("Kuria's text search: {e}")))?;
            found
                .into_iter()
                .map(|f| Match {
                    number: f.akt.number,
                    title: f.akt.title,
                    type_key: f.akt.akt_type.key,
                    field: f.field,
                    excerpt: f.excerpt,
                })
                .collect()
        }
        Source::Local(local) => local_matches(local, asked, limit as usize),
    };
    let found = super::find(source, asked.text, limit, asked.archived).await?;
    let by_title: Vec<Json> = found.json["akter"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| asked.akt_type.is_none_or(|key| row["type"] == key))
        .filter(|row| !matches.iter().any(|m| row["number"] == m.number.as_str()))
        .cloned()
        .collect();
    Ok(answer(&matches, &by_title))
}

/// The directory's text fields that hold every word of the text, ignoring
/// case, ranked by how many times they occur (`mcp.search`).
// r[impl cli.local.rules]
fn local_matches(local: &Local, asked: &Searching<'_>, limit: usize) -> Vec<Match> {
    let words: Vec<String> = asked
        .text
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let dir = &local.dir;
    let mut scored = Vec::new();
    for akt in dir.with(asked.archived) {
        if asked.akt_type.is_some_and(|key| key != akt.type_key) {
            continue;
        }
        let ty = &dir.type_of(akt).ty;
        for field in &ty.document().fields {
            if !matches!(field.kind, FieldKind::Text | FieldKind::Markdown) {
                continue;
            }
            let Some(text) = akt.values.get(&field.key).and_then(Json::as_str) else {
                continue;
            };
            let lower = text.to_lowercase();
            let counts: Vec<usize> = words
                .iter()
                .map(|w| lower.matches(w.as_str()).count())
                .collect();
            if counts.contains(&0) {
                continue;
            }
            scored.push((
                counts.iter().sum::<usize>(),
                Match {
                    number: akt.number.clone(),
                    title: answers::title(dir, akt),
                    type_key: akt.type_key.clone(),
                    field: field.key.clone(),
                    excerpt: excerpt(text, &words),
                },
            ));
        }
    }
    scored.sort_by(|(a, x), (b, y)| b.cmp(a).then_with(|| x.number.cmp(&y.number)));
    scored.into_iter().take(limit).map(|(_, m)| m).collect()
}

/// A part of `text` around the first of `words`, each occurrence of them
/// marked as Kuria marks them, « and ».
fn excerpt(text: &str, words: &[String]) -> String {
    const AROUND: usize = 80;
    let lower = text.to_lowercase();
    // Lowercasing can change byte lengths; mark only where it didn't.
    if lower.len() != text.len() {
        return text.chars().take(AROUND * 2).collect();
    }
    let first = words
        .iter()
        .filter_map(|w| lower.find(w.as_str()))
        .min()
        .unwrap_or(0);
    let start = floor(text, first.saturating_sub(AROUND));
    let end = floor(text, (first + AROUND).min(text.len()));
    let end = if end < text.len() { end } else { text.len() };
    let part = &text[start..end];
    let part_lower = &lower[start..end];
    let mut marked = String::with_capacity(part.len() + 8);
    let mut at = 0;
    while at < part.len() {
        let hit = words
            .iter()
            .filter(|w| !w.is_empty() && part_lower[at..].starts_with(w.as_str()))
            .map(String::len)
            .max();
        if let Some(len) = hit {
            marked.push('«');
            marked.push_str(&part[at..at + len]);
            marked.push('»');
            at += len;
        } else {
            let ch = part[at..].chars().next().unwrap_or(' ');
            marked.push(ch);
            at += ch.len_utf8();
        }
    }
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(marked.trim());
    if end < text.len() {
        out.push('…');
    }
    out.replace('\n', " ")
}

/// The character boundary at or before `at`.
fn floor(text: &str, at: usize) -> usize {
    (0..=at.min(text.len()))
        .rev()
        .find(|&i| text.is_char_boundary(i))
        .unwrap_or(0)
}

fn answer(matches: &[Match], by_title: &[Json]) -> Answer {
    let mut markdown = String::new();
    if matches.is_empty() && by_title.is_empty() {
        markdown.push_str("No akter.");
    }
    if !matches.is_empty() {
        let table: Vec<Vec<String>> = matches
            .iter()
            .map(|m| {
                vec![
                    m.number.clone(),
                    m.title.clone().unwrap_or_default(),
                    m.field.clone(),
                    m.excerpt.replace('|', "\\|"),
                ]
            })
            .collect();
        markdown.push_str(&md::table(&["Number", "Title", "Field", "Excerpt"], &table));
    }
    if !by_title.is_empty() {
        if !markdown.is_empty() {
            markdown.push('\n');
        }
        markdown.push_str("By number or title:\n\n");
        let table: Vec<Vec<String>> = by_title
            .iter()
            .map(|row| {
                vec![
                    row["number"].as_str().unwrap_or_default().to_owned(),
                    row["title"].as_str().unwrap_or_default().to_owned(),
                ]
            })
            .collect();
        markdown.push_str(&md::table(&["Number", "Title"], &table));
    }
    Answer {
        markdown,
        json: json!({
            "matches": matches.iter().map(|m| json!({
                "number": m.number, "title": m.title, "type": m.type_key,
                "field": m.field, "excerpt": m.excerpt,
            })).collect::<Vec<_>>(),
            "akter": by_title,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpts_mark_every_word_around_the_first() {
        let words = vec!["install".to_owned(), "restart".to_owned()];
        assert_eq!(
            excerpt("Run the installer, then restart.", &words),
            "Run the «install»er, then «restart»."
        );
        let long = format!("{}install it{}", "a".repeat(200), "b".repeat(200));
        let cut = excerpt(&long, &words);
        assert!(cut.starts_with('…') && cut.ends_with('…'), "{cut}");
        assert!(cut.contains("«install»"), "{cut}");
    }
}
