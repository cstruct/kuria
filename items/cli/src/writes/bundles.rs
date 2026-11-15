//! `export` and `import`: bundles of selected akt types or the whole
//! tenant, as directories (`bundle.format`).

use std::io::Read as _;
use std::path::Path;

use reqwest::Method;
use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::http::{Body, json as read_json};
use crate::output::Answer;
use crate::profile::io_error;
use crate::session::Session;

/// What to export.
pub enum Exported<'a> {
    Types { keys: &'a [String], akter: bool },
    Tenant,
}

/// A refusal Kuria answered with, as `{"error": {"code", "message"}}`.
async fn refused(response: reqwest::Response) -> CliError {
    let status = response.status();
    match read_json(response, "Kuria").await {
        Ok(body) => {
            let code = body["error"]["code"].as_str().unwrap_or("REFUSED");
            CliError::from_server(
                code,
                body["error"]["message"].as_str().unwrap_or(code),
                None,
            )
        }
        Err(_) if status.is_server_error() => {
            CliError::unavailable(format!("Kuria failed ({status})"))
        }
        Err(error) => error,
    }
}

/// The bundle Kuria exports for `query`, unpacked: `bundle.json` and each
/// attachment's bytes, by their names in the bundle.
pub async fn fetch(session: &Session, query: &str) -> Result<Vec<(String, Vec<u8>)>, CliError> {
    let response = session
        .http
        .send(
            Method::GET,
            &format!("{}/export?{query}", session.url),
            Some(session.access_token()),
            &Body::Empty,
        )
        .await?;
    if !response.status().is_success() {
        return Err(refused(response).await);
    }
    let archive = response.bytes().await?;
    let mut files = Vec::new();
    let mut entries = tar::Archive::new(archive.as_ref());
    let unreadable = |e: std::io::Error| {
        CliError::unavailable(format!("Kuria's bundle isn't a tar archive: {e}"))
    };
    for entry in entries.entries().map_err(unreadable)? {
        let mut entry = entry.map_err(unreadable)?;
        let name = entry
            .path()
            .map_err(unreadable)?
            .to_string_lossy()
            .into_owned();
        // Only what a bundle holds, never outside the directory.
        let safe = name == "bundle.json"
            || name
                .strip_prefix("attachments/")
                .is_some_and(|sha| !sha.is_empty() && sha.bytes().all(|b| b.is_ascii_hexdigit()));
        if !safe {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(unreadable)?;
        files.push((name, bytes));
    }
    Ok(files)
}

/// `bundle.json` of an unpacked bundle.
pub fn bundle_json(files: &[(String, Vec<u8>)]) -> Result<Json, CliError> {
    files
        .iter()
        .find(|(name, _)| name == "bundle.json")
        .and_then(|(_, b)| serde_json::from_slice(b).ok())
        .ok_or_else(|| CliError::unavailable("Kuria's bundle holds no bundle.json"))
}

/// Refuses `dir` unless it is new or empty.
pub fn check_new(dir: &Path) -> Result<(), CliError> {
    if dir.exists()
        && std::fs::read_dir(dir)
            .map_err(|e| io_error(dir, &e))?
            .next()
            .is_some()
    {
        return Err(CliError::refused(
            "NOT_EMPTY",
            format!("{} isn't empty; export into a new directory", dir.display()),
        ));
    }
    Ok(())
}

/// `kuria export (--types <key>,… [--akter] | --whole-tenant) -o <dir>`.
// r[impl cli.write]
pub async fn export(
    session: &Session,
    what: &Exported<'_>,
    dir: &Path,
) -> Result<Answer, CliError> {
    check_new(dir)?;
    let query = match what {
        Exported::Types { keys, akter } => format!("types={}&akter={akter}", keys.join(",")),
        Exported::Tenant => "tenant=true".to_owned(),
    };
    let files = fetch(session, &query).await?;
    let bundle = bundle_json(&files)?;
    for (name, bytes) in &files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_error(parent, &e))?;
        }
        std::fs::write(&path, bytes).map_err(|e| io_error(&path, &e))?;
    }
    let count = |key: &str| bundle[key].as_array().map_or(0, Vec::len);
    Ok(Answer {
        markdown: format!(
            "Exported {} akt types, {} akter, {} contacts and {} attachments to {}.",
            count("aktTypes"),
            count("akter"),
            count("contacts"),
            count("attachments"),
            dir.display()
        ),
        json: json!({
            "dir": dir,
            "scope": bundle["scope"],
            "aktTypes": count("aktTypes"),
            "akter": count("akter"),
            "contacts": count("contacts"),
            "attachments": count("attachments"),
            "files": files.len(),
        }),
    })
}

/// `kuria import <dir> [--map-subjects <file>]`: the bundle in `dir`, with
/// the bundle's subjects mapped to the tenant's by the JSON object in
/// `subjects`.
// r[impl cli.write]
pub async fn import(
    session: &Session,
    dir: &Path,
    subjects: Option<&Path>,
) -> Result<Answer, CliError> {
    let bundle = dir.join("bundle.json");
    if !bundle.is_file() {
        return Err(CliError::refused(
            "INVALID_BUNDLE",
            format!("{} holds no bundle.json", dir.display()),
        ));
    }
    let bytes = std::fs::read(&bundle).map_err(|e| io_error(&bundle, &e))?;
    let attachments = dir.join("attachments");
    let mut files = Vec::new();
    if attachments.is_dir() {
        for entry in std::fs::read_dir(&attachments).map_err(|e| io_error(&attachments, &e))? {
            let path = entry.map_err(|e| io_error(&attachments, &e))?.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let read = std::fs::read(&path).map_err(|e| io_error(&path, &e))?;
            files.push((format!("attachments/{name}"), read));
        }
    }
    post(session, &bytes, &files, subjects).await
}

/// Posts a bundle to Kuria: `bundle` as its `bundle.json`, `files` by
/// their names in it, such as `attachments/<sha256>`, and `subjects` as
/// its `subjects.json`.
pub async fn post(
    session: &Session,
    bundle: &[u8],
    files: &[(String, Vec<u8>)],
    subjects: Option<&Path>,
) -> Result<Answer, CliError> {
    let tarred = |e: std::io::Error| CliError::refused("IO", format!("packing a bundle: {e}"));
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_ustar();
    header.set_size(bundle.len() as u64);
    header.set_mode(0o644);
    builder
        .append_data(&mut header, "bundle.json", bundle)
        .map_err(tarred)?;
    for (name, bytes) in files {
        let mut header = tar::Header::new_ustar();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        builder
            .append_data(&mut header, name, bytes.as_slice())
            .map_err(tarred)?;
    }
    if let Some(path) = subjects {
        let mapping: Json = std::fs::read(path)
            .map_err(|e| io_error(path, &e))
            .and_then(|b| {
                serde_json::from_slice(&b).map_err(|e| {
                    CliError::refused("BAD_REQUEST", format!("{}: not JSON: {e}", path.display()))
                })
            })?;
        if !mapping.is_object() {
            return Err(CliError::refused(
                "BAD_REQUEST",
                format!(
                    "{} maps subjects to subjects, as a JSON object",
                    path.display()
                ),
            ));
        }
        let bytes = serde_json::to_vec(&mapping).expect("serializable");
        let mut header = tar::Header::new_ustar();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        builder
            .append_data(&mut header, "subjects.json", bytes.as_slice())
            .map_err(tarred)?;
    }
    let archive = builder.into_inner().map_err(tarred)?;
    let response = session
        .http
        .send(
            Method::POST,
            &format!("{}/import", session.url),
            Some(session.access_token()),
            &Body::Bytes(&archive, "application/x-tar"),
        )
        .await?;
    if !response.status().is_success() {
        return Err(refused(response).await);
    }
    let imported = read_json(response, "Kuria").await?;
    Ok(Answer {
        markdown: format!(
            "Imported {} akt types and {} akter ({}); {} subjects are shown as this tenant's users.",
            imported["aktTypes"],
            imported["akter"],
            imported["scope"].as_str().unwrap_or_default(),
            imported["aliases"]
        ),
        json: imported,
    })
}
