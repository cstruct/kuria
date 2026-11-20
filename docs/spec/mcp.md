# MCP server

`kuria mcp` is Kuria's MCP server: the command-line client ([cli](cli.md))
speaking the [Model Context Protocol](https://modelcontextprotocol.io) over
standard input and output, so that an agent on the person's machine, such
as Claude Code or a desktop app, searches, reads and edits akter through
tools. Kuria serves no MCP over HTTP.

## Running

r[mcp.stdio]
`kuria mcp` MUST serve MCP over standard input and output until its input
ends, writing nothing else to standard output. It MUST sign in as every
other command does (`cli.profile`, `cli.secret-key`, `cli.dpop`), and in
local mode (`cli.local.dir`) MUST work on the akt directory, by the same
rules and with the same answers, save what local mode lacks
(`cli.local.unsupported`).

r[mcp.tools]
It MUST offer the tools `search_documents`, `read_document`,
`edit_document`, `list_types` and `type_schema`, each describing its
arguments with a JSON Schema, and answer each call as the command it
stands for would, as text an agent reads (`cli.format`).

r[mcp.errors]
A refused call MUST be answered as a tool error, not a protocol error,
carrying the code, message and field the command would say
(`cli.errors`), so that the agent can act on it.

## Tools

r[mcp.search]
`search_documents` MUST take a text, and optionally an akt type and how
many to answer, and MUST answer the text search's matches
(`akt.search.text`), each with its akt's number, title and type, the field
and its excerpt, followed by the akter whose number or title match
(`akt.search`) that no text matched. In local mode, an akt's field MUST
match when it holds every word of the text, ignoring case, ranked by how
many times they occur.

r[mcp.read]
`read_document` MUST take an akt number, and optionally a field and a
heading id. It MUST answer the akt as `kuria cat` writes it (`cli.file`),
saying whether it has a live draft; with a field, only that field; with a
heading id, only that section of a `markdown` field, from its heading to
the next heading of the same or a higher level, refusing an id the field
has no heading for as `NOT_FOUND`.

r[mcp.edit]
`edit_document` MUST take an akt number, its replacements, each a field,
a span and what it becomes, and optionally a reason, and MUST apply them as
one change (`akt.replace`), answering for each field changed the text
around each span as it now reads. In local mode it MUST replace them in the
akt directory's file by the same rules (`akt.replace.match`).

r[mcp.types]
`list_types` MUST answer the akt types, as `kuria types` does, and
`type_schema` an akt type's fields, states and transitions, as `kuria types
schema` does.

## Presence

r[mcp.presence]
While `edit_document` changes an akt with a live draft through a server,
`kuria mcp` SHOULD join its live draft (`live.sync`) and set its awareness
to where each span it replaces is, so that the people editing see where the
agent works; the change itself MUST still be made by `akt.replace`.
