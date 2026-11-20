//! `types`, `schema`, `users` and `contacts`.

use serde_json::{Value as Json, json};

use super::Source;
use crate::error::CliError;
use crate::local;
use crate::md;
use crate::model::{FieldDef, Shape};
use crate::output::Answer;

/// `kuria types`: the akt types, with how many akter each has.
// r[impl cli.read]
pub async fn types(source: Source<'_>) -> Result<Answer, CliError> {
    let types = match source {
        Source::Server(session) => {
            let data = session
                .graphql(
                    "{ aktTypes { key name version numberPattern akterCount } }",
                    json!({}),
                )
                .await?;
            data["aktTypes"].as_array().cloned().unwrap_or_default()
        }
        Source::Local(l) => local::answers::types(&l.dir)
            .as_array()
            .cloned()
            .unwrap_or_default(),
    };
    let count = |t: &Json| {
        t["akterCount"]
            .as_i64()
            .map_or("—".to_owned(), |n| n.to_string())
    };
    let rows: Vec<Vec<String>> = types
        .iter()
        .map(|t| {
            let s = |k: &str| t[k].as_str().unwrap_or_default().to_owned();
            vec![
                format!("`{}`", s("key")),
                s("name"),
                t["version"].to_string(),
                format!("`{}`", s("numberPattern")),
                count(t),
            ]
        })
        .collect();
    let markdown = if rows.is_empty() {
        "No akt types.".to_owned()
    } else {
        md::table(&["Key", "Name", "Version", "Numbers", "Akter"], &rows)
    };
    Ok(Answer {
        markdown,
        json: json!({ "types": types.iter().map(|t| json!({
            "key": t["key"],
            "name": t["name"],
            "version": t["version"],
            "numberPattern": t["numberPattern"],
            "akterCount": t["akterCount"],
        })).collect::<Vec<_>>() }),
    })
}

/// A field's kind as the akt type document names it: `text`, `user`, ….
fn kind(field: &FieldDef) -> String {
    field.kind.to_ascii_lowercase()
}

/// `kuria types schema <type>`: everything a type says about its akter.
// r[impl cli.read]
pub async fn schema(source: Source<'_>, key: &str) -> Result<Answer, CliError> {
    let shape = super::shape(source, key).await?;
    Ok(Answer {
        markdown: schema_markdown(&shape),
        json: json!({ "type": schema_json(&shape) }),
    })
}

fn schema_json(shape: &Shape) -> Json {
    let document = &shape.document;
    json!({
        "key": shape.key,
        "name": shape.name,
        "version": shape.version,
        "numberPattern": shape.number_pattern,
        "fields": shape.fields.iter().map(|f| json!({
            "key": f.key,
            "name": f.name,
            "kind": kind(f),
            "required": f.required,
            "multiline": f.multiline,
            "references": f.references,
            "description": f.description,
            "options": f.options.iter().map(|o| json!({ "key": o.key, "name": o.name })).collect::<Vec<_>>(),
            "format": f.format.as_deref().map(str::to_ascii_lowercase),
            "draft": document["fields"].as_array().and_then(|fs| fs.iter().find(|d| d["key"] == f.key.as_str())).map(|d| d["draft"].clone()).filter(|d| !d.is_null()),
        })).collect::<Vec<_>>(),
        "states": shape.states.iter().map(|s| json!({
            "key": s.key,
            "name": s.name,
            "initial": s.initial,
            "final": s.is_final,
            "tone": s.tone.as_deref().map(str::to_ascii_lowercase),
        })).collect::<Vec<_>>(),
        "transitions": shape.transitions.iter().map(|t| json!({
            "key": t.key,
            "name": t.name,
            "from": t.from,
            "to": t.to,
            "requires": t.requires,
            "outcomes": t.outcomes.iter().map(|o| o.to_ascii_lowercase()).collect::<Vec<_>>(),
            "action": document["transitions"].as_array().and_then(|ts| ts.iter().find(|d| d["key"] == t.key.as_str())).and_then(|d| d["action"].as_str()),
            "waitsFor": waits_of(document, &t.key),
            "requiresLinks": requires_links_of(document, &t.key),
        })).collect::<Vec<_>>(),
        "contains": document.get("contains").cloned().unwrap_or_else(|| json!([])),
        "createdIn": document.get("created_in").cloned().unwrap_or_else(|| json!([])),
        "links": document.get("links").cloned().unwrap_or_else(|| json!([])),
        "board": document.get("board").cloned(),
        "presentation": {
            "title": shape.presentation.title,
            "card": shape.presentation.card,
            "list": shape.presentation.list,
            "icon": shape.presentation.icon.as_deref().map(str::to_ascii_lowercase),
            "colour": shape.presentation.colour.to_ascii_lowercase(),
        },
    })
}

fn schema_markdown(shape: &Shape) -> String {
    let document = &shape.document;
    // r[impl akt.number.shared]: the sequence it shares, if it names one.
    let shared = document["sequence"]
        .as_str()
        .map(|sequence| format!(", from the sequence `{sequence}`"))
        .unwrap_or_default();
    let mut text = format!(
        "# {} (`{}`, version {})\n\nNumbers: `{}`{shared}\n\n## Fields\n\n",
        shape.name, shape.key, shape.version, shape.number_pattern
    );
    let rows: Vec<Vec<String>> = shape
        .fields
        .iter()
        .map(|f| {
            let mut kind = kind(f);
            if f.multiline {
                kind.push_str(", multiline");
            }
            if let Some(references) = &f.references {
                kind.push_str(&format!(", references kept as `{references}` links"));
            }
            if let Some(format) = &f.format {
                kind.push_str(&format!(", {}", format.to_ascii_lowercase()));
            }
            let options = f
                .options
                .iter()
                .map(|o| format!("`{}` {}", o.key, o.name))
                .collect::<Vec<_>>()
                .join(", ");
            vec![
                format!("`{}`", f.key),
                f.name.clone(),
                kind,
                if f.required { "yes" } else { "" }.to_owned(),
                options,
                f.description.clone().unwrap_or_default(),
            ]
        })
        .collect();
    text.push_str(&md::table(
        &["Key", "Name", "Kind", "Required", "Options", "Description"],
        &rows,
    ));
    text.push_str("\n## States\n\n");
    for s in &shape.states {
        let mut notes = Vec::new();
        if s.initial {
            notes.push("initial".to_owned());
        }
        if s.is_final {
            notes.push("final".to_owned());
        }
        if let Some(tone) = s.tone.as_deref().filter(|t| *t != "NEUTRAL") {
            notes.push(tone.to_ascii_lowercase());
        }
        let notes = if notes.is_empty() {
            String::new()
        } else {
            format!(" ({})", notes.join(", "))
        };
        text.push_str(&format!("- `{}` {}{notes}\n", s.key, s.name));
    }
    if !shape.transitions.is_empty() {
        text.push_str("\n## Transitions\n\n");
        for t in &shape.transitions {
            let mut line = format!("- `{}` {}: {} → {}", t.key, t.name, t.from.join(", "), t.to);
            if !t.requires.is_empty() {
                line.push_str(&format!(", requires {}", t.requires.join(", ")));
            }
            // r[impl akt.transition.outcomes]
            if !t.outcomes.is_empty() {
                let outcomes: Vec<String> =
                    t.outcomes.iter().map(|o| o.to_ascii_lowercase()).collect();
                line.push_str(&format!(", only if the outcome is {}", outcomes.join(", ")));
            }
            line.push_str(&waits_text(document, &t.key));
            text.push_str(&line);
            text.push('\n');
        }
    }
    text.push_str(&document_markdown(document));
    let p = &shape.presentation;
    text.push_str("\n## Presentation\n\n");
    text.push_str(&format!(
        "- Title: {}\n- Cards: {}\n- Lists: {}\n",
        p.title.as_deref().unwrap_or("—"),
        p.card.join(", "),
        p.list.join(", ")
    ));
    text
}

/// What the document says that the shape doesn't: what the type contains
/// and links to, and its board.
fn document_markdown(document: &Json) -> String {
    let mut text = String::new();
    if let Some(contains) = document["contains"].as_array().filter(|c| !c.is_empty()) {
        text.push_str("\n## Contains\n\n");
        for c in contains {
            let mut line = format!("- `{}`", c["type"].as_str().unwrap_or_default());
            if c["exclusive"].as_bool() == Some(true) {
                line.push_str(", exclusive");
            }
            if let Some(states) = c["while"].as_array() {
                let states: Vec<&str> = states.iter().filter_map(Json::as_str).collect();
                line.push_str(&format!(", while {}", states.join(", ")));
            }
            text.push_str(&line);
            text.push('\n');
        }
    }
    if let Some(links) = document["links"].as_array().filter(|l| !l.is_empty()) {
        text.push_str("\n## Links\n\n");
        for l in links {
            let to: Vec<&str> = l["to"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Json::as_str)
                .collect();
            text.push_str(&format!(
                "- `{}` {} (inverse: {}), to {}\n",
                l["key"].as_str().unwrap_or_default(),
                l["name"].as_str().unwrap_or_default(),
                l["inverse"].as_str().unwrap_or_default(),
                to.join(", ")
            ));
        }
    }
    if let Some(columns) = document["board"]["columns"].as_array() {
        text.push_str("\n## Board\n\n");
        for c in columns {
            let states: Vec<String> = c["states"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(t, s)| {
                    let s: Vec<&str> = s
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Json::as_str)
                        .collect();
                    format!("{t}: {}", s.join(", "))
                })
                .collect();
            text.push_str(&format!(
                "- `{}` {} ({})\n",
                c["key"].as_str().unwrap_or_default(),
                c["name"].as_str().unwrap_or_default(),
                states.join("; ")
            ));
        }
        // r[impl board.parent]
        if let Some(parent) = document["board"]["parent"].as_str() {
            text.push_str(&format!("\nParent boards: its `{parent}` links\n"));
        }
    }
    text
}

/// `kuria users`: whom user fields can name.
// r[impl cli.read]
pub async fn users(source: Source<'_>) -> Result<Answer, CliError> {
    let Source::Server(session) = source else {
        return Err(local::unsupported(
            "`users`",
            "an akt directory names people as text",
        ));
    };
    let data = session
        .graphql("{ users { id name email } }", json!({}))
        .await?;
    let users = data["users"].as_array().cloned().unwrap_or_default();
    let rows: Vec<Vec<String>> = users
        .iter()
        .map(|u| {
            let s = |k: &str| u[k].as_str().unwrap_or_default().to_owned();
            vec![s("name"), s("email"), format!("`{}`", s("id"))]
        })
        .collect();
    Ok(Answer {
        markdown: if rows.is_empty() {
            "No users have signed in.".to_owned()
        } else {
            md::table(&["Name", "Email", "Id"], &rows)
        },
        json: json!({ "users": users }),
    })
}

/// `kuria contacts`: the people values can name who haven't signed in.
// r[impl cli.read]
pub async fn contacts(source: Source<'_>) -> Result<Answer, CliError> {
    let Source::Server(session) = source else {
        return Err(local::unsupported(
            "`contacts`",
            "an akt directory names people as text",
        ));
    };
    let data = session
        .graphql(
            "{ contacts { id displayName name email user { id name email } } }",
            json!({}),
        )
        .await?;
    let contacts = data["contacts"].as_array().cloned().unwrap_or_default();
    let rows: Vec<Vec<String>> = contacts
        .iter()
        .map(|c| {
            let s = |k: &str| c[k].as_str().unwrap_or_default().to_owned();
            vec![
                s("displayName"),
                s("email"),
                c["user"]["name"].as_str().unwrap_or_default().to_owned(),
                format!("`contact:{}`", s("id")),
            ]
        })
        .collect();
    Ok(Answer {
        markdown: if rows.is_empty() {
            "No contacts.".to_owned()
        } else {
            md::table(&["Name", "Email", "Linked to", "Id"], &rows)
        },
        json: json!({ "contacts": contacts }),
    })
}

/// What the transition `key` waits for and the links it needs, as the
/// schema's line for it goes on: ", waits for its `action` members to be
/// final", ….
// r[impl akt.transition.members]
// r[impl akt.transition.links]
fn waits_text(document: &Json, key: &str) -> String {
    let keys = |list: &Json| -> Vec<String> {
        list.as_array()
            .into_iter()
            .flatten()
            .filter_map(|k| k.as_str().map(|k| format!("`{k}`")))
            .collect()
    };
    let waits = waits_of(document, key);
    let mut text = String::new();
    let members = keys(&waits["members"]);
    if !members.is_empty() {
        text.push_str(&format!(
            ", waits for its {} members to be final",
            members.join(", ")
        ));
    }
    let links = keys(&waits["links"]);
    if !links.is_empty() {
        text.push_str(&format!(
            ", waits for its {} links' akter to be final",
            links.join(", ")
        ));
    }
    let needs = keys(&requires_links_of(document, key));
    if !needs.is_empty() {
        text.push_str(&format!(", needs a {} link", needs.join(", ")));
    }
    text
}

/// The transition `key`'s definition in `document`, if it has one.
fn transition_def<'a>(document: &'a Json, key: &str) -> Option<&'a Json> {
    document["transitions"]
        .as_array()
        .and_then(|ts| ts.iter().find(|d| d["key"] == key))
}

/// What the transition `key` waits for, as `{members, links}`.
fn waits_of(document: &Json, key: &str) -> Json {
    let waits = transition_def(document, key).map(|d| &d["waits_for"]);
    let list = |k: &str| {
        waits
            .and_then(|w| w.get(k))
            .cloned()
            .unwrap_or_else(|| json!([]))
    };
    json!({ "members": list("members"), "links": list("links") })
}

/// The link kinds the transition `key` requires.
fn requires_links_of(document: &Json, key: &str) -> Json {
    transition_def(document, key)
        .and_then(|d| d.get("requires_links"))
        .cloned()
        .unwrap_or_else(|| json!([]))
}
