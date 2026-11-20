---
number: PLAN-028
type: plan
revision: 5
rank: b63
state: dropped
title: "Remote MCP served by Kuria, for hosted agents such as claude.ai"
owner: albin@mattsson.io
---

<!-- field: goal -->
Hosted agents, such as a claude.ai connector, search and edit Kuria's documents through MCP over HTTP, served by Kuria itself, with the same tools, checks and attribution as `kuria mcp` (PLAN-027). A person adds Kuria as a connector, signs in through Konklav in the browser, picks a tenant, and the agent's edits read as theirs, through the agent. You know it works when a claude.ai connector pointed at a local Kuria finds a document by a phrase in its body and changes it while someone types in it.

<!-- field: design -->
Builds on PLAN-027: its tools, `replaceInAkt`, search through pg_search and attribution from Konklav's actor chain (PLAN-027-D04). What remote MCP asks of Kuria and Konklav is recorded in PLAN-027-G03.

## Constraints And Assumptions

- MCP specification 2026-07-28: Streamable HTTP, one `POST` endpoint, no sessions; Protected Resource Metadata (RFC 9728); bearer tokens; RFC 8707's `resource` sent by clients; client ids by pre-registration, Client ID Metadata Documents, or (deprecated) dynamic registration.
- Konklav is the authorization server: its OpenID provider has discovery, the code flow with PKCE and tenant selection, but clients are registered in its control plane only.

## Design

- `/mcp` on the server: the tools of PLAN-027's shared crate, run in-process as the caller against the GraphQL schema, so authorization and checks are a request's.
- `/.well-known/oauth-protected-resource` (and a 401's `WWW-Authenticate` naming it) pointing at Konklav's issuer, with the scopes to ask for (`openid`, a tenant).
- Tokens issued to an MCP client have Kuria's audience and name the client in `act`, so attribution follows PLAN-027-D04.

<!-- field: verification -->
- Server tests: the metadata, a 401 challenge, a tool call with a token issued to a registered client, one with another audience refused.
- By hand: a claude.ai connector (or the MCP Inspector) against a local stack.

<!-- akt: PLAN-028-G01 -->
---
type: gap
revision: 1
rank: b64
state: open
in: [PLAN-028]
title: How a hosted client gets a client id from Konklav
---

<!-- field: description -->
Konklav registers clients in its control plane only. Until it supports Client ID Metadata Documents, an operator registers each host and the person enters its client id; whether claude.ai lets them isn't verified.

<!-- akt: PLAN-028-G02 -->
---
type: gap
revision: 1
rank: b65
state: open
in: [PLAN-028]
title: "Whether Konklav accepts RFC 8707's resource parameter"
---

<!-- field: description -->
MCP clients send `resource` in every authorization and token request, whether the server supports it or not. Not verified whether Konklav ignores it or refuses the request.

<!-- akt: PLAN-028-G03 -->
---
type: gap
revision: 1
rank: b66
state: open
in: [PLAN-028]
title: Whether tokens issued to a client name it in act
---

<!-- field: description -->
Konklav's OIDC clients are service principals, but their tokens name only the person. Naming the client in `act` would give hosted agents PLAN-027-D04's attribution; it may also be how PLAN-027-T09 delegates.
