//! `copy` in local mode (`akt.copy`): an akt and the part of its member
//! tree chosen, as new akter of the same types in their initial states,
//! made as creates are, under the directory's lock.

use std::collections::{HashMap, HashSet};

use kuria_config::FieldKind;
use kuria_engine::{Value, copying, sections};
use serde_json::{Map, Value as Json};

use super::answers;
use super::arrange::add_link;
use super::edits::create_with;
use super::values::{after, current};
use super::write::{Writer, answer};
use crate::error::CliError;
use crate::output::Answer;

/// The akt `number` and every member under it, each once, under the first
/// container it is found in: parents first, in the directory's order.
fn tree(w: &Writer, number: &str) -> Result<Vec<(String, Option<String>)>, CliError> {
    let root = w.akt(number)?;
    let mut nodes = vec![(root.number.clone(), None)];
    let mut seen = HashSet::from([root.number.clone()]);
    let mut at = 0;
    while at < nodes.len() {
        let container = nodes[at].0.clone();
        for member in w.dir.members_of(&container) {
            if seen.insert(member.number.clone()) {
                nodes.push((member.number.clone(), Some(container.clone())));
            }
        }
        at += 1;
    }
    Ok(nodes)
}

/// `kuria copy <number> [--only <number>…] [--in <container>…]` in local
/// mode.
// r[impl akt.copy]
// r[impl akt.copy.links]
// r[impl cli.local.rules]
pub fn copy(
    w: &mut Writer,
    number: &str,
    only: &[String],
    inside: &[String],
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let nodes = tree(w, number)?;
    let pairs: Vec<(&str, Option<&str>)> = nodes
        .iter()
        .map(|(n, p)| (n.as_str(), p.as_deref()))
        .collect();
    let kept = copying::kept(&pairs, (!only.is_empty()).then_some(only))
        .map_err(|e| CliError::from_server("BAD_REQUEST", e, None))?;
    let nodes: Vec<(String, Option<String>)> = nodes
        .into_iter()
        .filter(|(n, _)| kept.contains(n))
        .collect();
    let inside: Vec<String> = if inside.is_empty() {
        w.akt(number)?.containers.clone()
    } else {
        inside.to_vec()
    };

    // Every link out of the kept akter, before any copy exists.
    let mut out: Vec<(String, String, String, Option<i32>)> = Vec::new();
    for (n, _) in &nodes {
        for link in answers::links(&w.dir, w.akt(n)?)? {
            if link["outgoing"] == true {
                out.push((
                    n.clone(),
                    link["kind"].as_str().unwrap_or_default().to_owned(),
                    link["akt"]["number"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    link["pinnedRevision"]
                        .as_i64()
                        .and_then(|r| i32::try_from(r).ok()),
                ));
            }
        }
    }

    // Each copy, parents first, each in its parent's copy.
    let mut copies: HashMap<String, String> = HashMap::new();
    for (n, parent) in &nodes {
        let akt = w.akt(n)?.clone();
        let ty = w.dir.type_of(&akt).ty.clone();
        let fields: Map<String, Json> = current(&ty, &akt, &mut w.ids)?
            .into_iter()
            .filter(|(key, _)| ty.field(key).is_some_and(|f| copying::keeps(f.kind)))
            .map(|(key, value)| (key, value.to_json()))
            .collect();
        let into = match parent {
            None => inside.clone(),
            Some(p) => vec![copies[p].clone()],
        };
        let made = create_with(w, &akt.type_key, &fields, &into, None)?;
        copies.insert(n.clone(), made);
    }

    // Each copy's sections, once the members they place exist.
    for (n, _) in &nodes {
        let akt = w.akt(n)?.clone();
        let ty = w.dir.type_of(&akt).ty.clone();
        let Some(field) = ty
            .document()
            .fields
            .iter()
            .find(|f| f.kind == FieldKind::Sections)
        else {
            continue;
        };
        let Some(raw) = akt.values.get(&field.key) else {
            continue;
        };
        let Ok(Value::Sections(items)) = Value::from_json(field, raw) else {
            continue;
        };
        let copied = sections::copied(&items, &|x| copies.contains_key(x), &|x| copies[x].clone());
        if copied.is_empty() {
            continue;
        }
        let copy = copies[n].clone();
        let mut fields = Map::new();
        fields.insert(field.key.clone(), Value::Sections(copied).to_json());
        let now = w.current(&copy)?;
        let change = w.change();
        let outcome = kuria_engine::edit(&ty, &now, &fields, change)
            .map_err(|r| CliError::from_server("REFUSED", r.to_string(), None))?;
        w.check_sections(&ty, Some(&copy), &outcome.changes)?;
        let changed: Vec<String> = outcome.changes.keys().cloned().collect();
        let values = after(w.dir.akt(&copy), &outcome.fields, &changed, &w.ids);
        w.akt_mut(&copy).values = values;
        w.bump(&copy);
    }

    // The links: between the copies within the tree, to the same akt, with
    // its pin, outside it.
    for (source, kind, target, pin) in out {
        let (to, pin) = match copies.get(&target) {
            Some(copy) => (copy.clone(), None),
            None => (target, pin),
        };
        add_link(w, &copies[&source], &kind, &to, pin)?;
    }

    let members: Vec<(String, String)> = nodes[1..]
        .iter()
        .map(|(n, _)| (n.clone(), copies[n].clone()))
        .collect();
    let (markdown, json) = crate::writes::copy::answer(number, &copies[number], &members);
    Ok(answer(markdown, json, reason))
}
