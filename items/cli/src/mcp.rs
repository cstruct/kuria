//! `kuria mcp`: the MCP server ([spec](../../../docs/spec/mcp.md)).
//!
//! It speaks MCP over standard input and output with `rmcp`, and answers
//! each tool call by running the command the tool stands for, as `kuria`
//! would with the same `--url`, `--tenant`, `--profile` and `--dir`: signed
//! in as the client is, or in local mode on the akt directory. A refusal
//! is a tool error carrying the command's code, message and field.

use std::path::PathBuf;
use std::process::ExitCode;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt};
use serde_json::{Map, Value as Json, json};
use uuid::Uuid;

use crate::args::{Cli, Command, TypesCommand};
use crate::error::CliError;
use crate::output::{Answer, Format};

/// Where the server's commands go: what `--url`, `--tenant`, `--profile`
/// and `--dir` said.
#[derive(Clone, Debug, Default)]
pub struct Target {
    pub url: Option<String>,
    pub tenant: Option<Uuid>,
    pub profile: Option<String>,
    pub dir: Option<PathBuf>,
}

impl Target {
    #[must_use]
    pub fn of(cli: &Cli) -> Self {
        Self {
            url: cli.url.clone(),
            tenant: cli.tenant,
            profile: cli.profile.clone(),
            dir: cli.dir.clone(),
        }
    }

    fn cli(&self, command: Command) -> Cli {
        Cli {
            format: Format::Md,
            url: self.url.clone(),
            tenant: self.tenant,
            profile: self.profile.clone(),
            dir: self.dir.clone(),
            command,
        }
    }
}

/// Serves MCP over standard input and output until its input ends.
// r[impl mcp.stdio]
pub async fn serve(target: Target) -> ExitCode {
    let served = tokio::task::LocalSet::new()
        .run_until(Box::pin(async move {
            let running = Kuria::new(target).serve(rmcp::transport::stdio()).await?;
            running
                .waiting()
                .await
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok::<_, Box<dyn std::error::Error>>(())
        }))
        .await;
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kuria mcp: {e}");
            ExitCode::from(5)
        }
    }
}

/// A tool call's arguments.
struct Arguments<'a>(&'a Map<String, Json>);

impl Arguments<'_> {
    fn text(&self, key: &str) -> Option<String> {
        self.0.get(key).and_then(Json::as_str).map(str::to_owned)
    }

    fn required(&self, key: &str) -> Result<String, CliError> {
        self.text(key).ok_or_else(|| CliError {
            field: Some(key.to_owned()),
            ..CliError::refused("BAD_REQUEST", format!("`{key}` is required"))
        })
    }
}

/// The server: its tools, answered by commands on `target`.
pub struct Kuria {
    target: Target,
    /// A session to run commands on instead of signing in for each.
    session: Option<crate::session::Session>,
}

impl Kuria {
    /// The server for commands on `target`, signing in for each as `kuria`
    /// does.
    #[must_use]
    pub fn new(target: Target) -> Self {
        Self {
            target,
            session: None,
        }
    }

    /// The server for commands through `session`, already open.
    #[must_use]
    pub fn on(session: crate::session::Session) -> Self {
        Self {
            target: Target::default(),
            session: Some(session),
        }
    }
}

const INSTRUCTIONS: &str = "Kuria keeps akter: records such as documents, tasks and CAPAs. \
Find documents with search_documents, read one with read_document (a field, or one section \
by its heading id, for long ones), and change text with edit_document: each replacement is \
a span of a field's text, as you read it, and what it becomes. Kuria finds each span in the \
field as it is when it applies the change, so what others typed elsewhere meanwhile is kept; \
a span someone changed is refused as NO_MATCH, and one that occurs more than once as \
AMBIGUOUS: read the field again, or give a longer span. list_types and type_schema say \
what fields each akt type has.";

impl ServerHandler for Kuria {
    fn get_info(&self) -> ServerConfig {
        let mut config = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        config.server_info = Implementation::new("kuria", env!("CARGO_PKG_VERSION"));
        config.instructions = Some(INSTRUCTIONS.to_owned());
        config
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "the trait's method is async"
    )]
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        serde_json::from_value(json!({ "tools": tools() }))
            .map_err(|e| McpError::internal_error(e.to_string(), None))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let arguments = request.arguments.unwrap_or_default();
        let result = match self.call(&request.name, &arguments).await {
            Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
            // r[impl mcp.errors]
            Err(error) => json!({
                "content": [{ "type": "text", "text": format!(
                    "{}\n\n{}",
                    error.message,
                    json!({ "error": { "code": error.code, "message": error.message, "field": error.field } })
                ) }],
                "isError": true,
            }),
        };
        let result: CallToolResult = serde_json::from_value(result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(result.into())
    }
}

impl Kuria {
    /// Runs `command` as `kuria` would, answering its Markdown.
    async fn run(&self, command: Command) -> Result<Answer, CliError> {
        match &self.session {
            Some(session) => crate::answer_on(session, command).await,
            None => crate::answer(self.target.cli(command)).await,
        }
    }

    /// The tool `name`'s answer to `arguments`.
    // r[impl mcp.tools]
    async fn call(&self, name: &str, arguments: &Map<String, Json>) -> Result<String, CliError> {
        let given = Arguments(arguments);
        match name {
            // r[impl mcp.search]
            "search_documents" => {
                let archived = given.text("archived");
                let limit = arguments
                    .get("limit")
                    .and_then(Json::as_u64)
                    .map_or(20, |n| u32::try_from(n).unwrap_or(50));
                let command = Command::Search {
                    text: vec![given.required("text")?],
                    akt_type: given.text("type"),
                    limit,
                    archived: archived.as_deref() == Some("archived"),
                    with_archived: archived.as_deref() == Some("all"),
                };
                Ok(self.run(command).await?.markdown)
            }
            "read_document" => self.read(&given).await,
            // r[impl mcp.edit]
            "edit_document" => {
                let mut fields = Vec::new();
                let mut spans = Vec::new();
                let mut texts = Vec::new();
                for r in arguments
                    .get("replacements")
                    .and_then(Json::as_array)
                    .into_iter()
                    .flatten()
                {
                    let part = |key: &str| {
                        r.get(key)
                            .and_then(Json::as_str)
                            .unwrap_or_default()
                            .to_owned()
                    };
                    fields.push(part("field"));
                    spans.push(part("span"));
                    texts.push(part("text"));
                }
                let command = Command::Replace {
                    number: given.required("number")?,
                    fields,
                    spans,
                    texts,
                    reason: given.text("reason"),
                };
                Ok(self.run(command).await?.markdown)
            }
            // r[impl mcp.types]
            "list_types" => Ok(self.run(Command::Types { action: None }).await?.markdown),
            "type_schema" => Ok(self
                .run(Command::Types {
                    action: Some(TypesCommand::Schema {
                        key: given.required("type")?,
                    }),
                })
                .await?
                .markdown),
            other => Err(CliError::refused(
                "BAD_REQUEST",
                format!("there is no tool `{other}`"),
            )),
        }
    }

    /// `read_document`: the akt file, or one field, or one section of it.
    // r[impl mcp.read]
    async fn read(&self, given: &Arguments<'_>) -> Result<String, CliError> {
        let number = given.required("number")?;
        let answer = self
            .run(Command::Cat {
                numbers: vec![number.clone()],
                output: None,
                members: Vec::new(),
                all_members: false,
                version: None,
                archived: false,
                with_archived: true,
            })
            .await?;
        let live = self.live(&answer.markdown).await;
        let body = match (given.text("field"), given.text("heading")) {
            (None, None) => answer.markdown,
            (Some(field), heading) => {
                let value = field_of(&answer.markdown, &field).ok_or_else(|| CliError {
                    field: Some(field.clone()),
                    ..CliError::from_server(
                        "NOT_FOUND",
                        format!("{number} has no value for `{field}`"),
                        None,
                    )
                })?;
                match heading {
                    None => value,
                    Some(id) => section_of(&value, &id).ok_or_else(|| {
                        CliError::from_server(
                            "NOT_FOUND",
                            format!("`{field}` of {number} has no heading `{{#{id}}}`"),
                            None,
                        )
                    })?,
                }
            }
            (None, Some(_)) => {
                return Err(CliError {
                    field: Some("field".to_owned()),
                    ..CliError::refused(
                        "BAD_REQUEST",
                        "a heading is one of a field's: name the field",
                    )
                });
            }
        };
        Ok(format!(
            "{number} {}.\n\n{body}",
            if live {
                "has a live draft: edits merge with what others type, and its text is the draft's"
            } else {
                "has no live draft: each edit makes a revision"
            }
        ))
    }

    /// Whether the akt that `file`, its akt file, holds has a live draft:
    /// its type's document says `live_drafts`.
    async fn live(&self, file: &str) -> bool {
        let Some(key) = front_matter(file, "type") else {
            return false;
        };
        let exported = self
            .run(Command::Types {
                action: Some(TypesCommand::Export {
                    keys: vec![key],
                    output: None,
                }),
            })
            .await;
        exported.is_ok_and(|a| a.markdown.contains("\"live_drafts\": true"))
    }
}

/// The value of the single-line key `key` in `file`'s front matter.
fn front_matter(file: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    file.lines()
        .skip(1)
        .take_while(|l| *l != "---")
        .find_map(|l| l.strip_prefix(&prefix))
        .map(|v| v.trim().trim_matches('"').to_owned())
}

/// The field `key` of the akt file `file`: its section, or its front
/// matter line.
fn field_of(file: &str, key: &str) -> Option<String> {
    let marker = format!("<!-- field: {key} -->");
    if let Some(at) = file.find(&marker) {
        let rest = &file[at + marker.len()..];
        let end = ["\n<!-- field: ", "\n<!-- akt: "]
            .iter()
            .filter_map(|m| rest.find(m))
            .min()
            .unwrap_or(rest.len());
        return Some(rest[..end].trim_matches('\n').to_owned());
    }
    front_matter(file, key).map(|value| format!("{key}: {value}"))
}

/// The section of `markdown` that the heading with the id `id` begins:
/// from it to the next heading of its level or higher, outside code.
fn section_of(markdown: &str, id: &str) -> Option<String> {
    let anchor = format!("{{#{id}}}");
    let level = |line: &str| {
        let hashes = line.chars().take_while(|c| *c == '#').count();
        (hashes > 0 && line[hashes..].starts_with(' ')).then_some(hashes)
    };
    let mut fenced = false;
    let mut start: Option<(usize, usize)> = None;
    let mut lines = Vec::new();
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        if !fenced && let Some(depth) = level(line) {
            match start {
                Some((top, _)) if depth <= top => break,
                None if line.trim_end().ends_with(&anchor) => start = Some((depth, 0)),
                _ => {}
            }
        }
        if start.is_some() {
            lines.push(line);
        }
    }
    start.map(|_| lines.join("\n"))
}

/// The tools, with their arguments' JSON Schemas (`mcp.tools`).
fn tools() -> Json {
    let archived = json!({
        "type": "string", "enum": ["live", "archived", "all"],
        "description": "Live akter (the default), archived ones, or both."
    });
    json!([
        {
            "name": "search_documents",
            "description": "Find akter by the words their text fields hold, the most relevant first, each with the field matched and an excerpt marking the words with « and »; then those whose number starts with the text or whose title holds its words.",
            "inputSchema": { "type": "object", "properties": {
                "text": { "type": "string", "description": "The words to find; every one must be in a field." },
                "type": { "type": "string", "description": "Only this akt type's akter, by key." },
                "limit": { "type": "integer", "minimum": 1, "maximum": 50, "description": "How many matches, 20 by default." },
                "archived": archived,
            }, "required": ["text"] },
            "annotations": { "readOnlyHint": true },
        },
        {
            "name": "read_document",
            "description": "Read an akt as an akt file: Markdown with its single-line fields in YAML front matter and each multiline field after `<!-- field: <key> -->`. Give `field` for one field, and `heading` for one section of a markdown field, by its heading id (`## Install {#install}` has the id `install`).",
            "inputSchema": { "type": "object", "properties": {
                "number": { "type": "string", "description": "The akt's number, such as DOC-12." },
                "field": { "type": "string", "description": "Only this field, by key." },
                "heading": { "type": "string", "description": "Only the section this heading id begins, of `field`." },
            }, "required": ["number"] },
            "annotations": { "readOnlyHint": true },
        },
        {
            "name": "edit_document",
            "description": "Change text fields by replacing spans: each `span` must occur exactly once in its field when Kuria applies it, and becomes `text`. Copy spans from what you read; what others changed elsewhere in the field since is kept. A span not found is refused as NO_MATCH, one found more than once as AMBIGUOUS: read again, or give a longer span. All replacements are one change.",
            "inputSchema": { "type": "object", "properties": {
                "number": { "type": "string", "description": "The akt's number." },
                "replacements": { "type": "array", "minItems": 1, "items": {
                    "type": "object", "properties": {
                        "field": { "type": "string", "description": "The text or markdown field, by key." },
                        "span": { "type": "string", "description": "The exact text to replace." },
                        "text": { "type": "string", "description": "What it becomes; empty deletes it." },
                    }, "required": ["field", "span", "text"] } },
                "reason": { "type": "string", "description": "Why, recorded with the change." },
            }, "required": ["number", "replacements"] },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false },
        },
        {
            "name": "list_types",
            "description": "The akt types, with how many akter each has.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true },
        },
        {
            "name": "type_schema",
            "description": "An akt type's fields (keys, kinds, options), states and transitions.",
            "inputSchema": { "type": "object", "properties": {
                "type": { "type": "string", "description": "The akt type's key." },
            }, "required": ["type"] },
            "annotations": { "readOnlyHint": true },
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "---\nnumber: DOC-1\ntype: guide\nrevision: 1\ntitle: \"Setup\"\n---\n\n<!-- field: body -->\n# Setup {#setup}\n\nIntro.\n\n## Install {#install}\n\nRun it.\n\n```\n# not a heading\n```\n\n### Details {#details}\n\nMore.\n\n## Restart {#restart}\n\nAgain.\n\n<!-- field: notes -->\nA note.\n";

    // r[verify mcp.read]
    #[test]
    fn fields_and_sections_are_cut_from_the_akt_file() {
        assert_eq!(front_matter(FILE, "type").as_deref(), Some("guide"));
        assert_eq!(field_of(FILE, "title").as_deref(), Some("title: Setup"));
        assert_eq!(field_of(FILE, "notes").as_deref(), Some("A note."));
        let body = field_of(FILE, "body").unwrap();
        assert!(body.starts_with("# Setup {#setup}") && body.ends_with("Again."));
        assert_eq!(
            section_of(&body, "install").unwrap(),
            "## Install {#install}\n\nRun it.\n\n```\n# not a heading\n```\n\n### Details {#details}\n\nMore.\n"
        );
        assert_eq!(
            section_of(&body, "restart").unwrap(),
            "## Restart {#restart}\n\nAgain."
        );
        assert_eq!(section_of(&body, "nope"), None);
    }

    // r[verify mcp.tools]
    #[test]
    fn every_tool_says_its_arguments() {
        let tools = tools();
        let names: Vec<&str> = tools
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "search_documents",
                "read_document",
                "edit_document",
                "list_types",
                "type_schema"
            ]
        );
        let parsed: ListToolsResult = serde_json::from_value(json!({ "tools": tools })).unwrap();
        assert_eq!(parsed.tools.len(), 5);
    }
}
