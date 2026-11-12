# Authentication

Kuria is a Konklav relying system: Konklav authenticates people and software
and publishes what they may do, and Kuria enforces that on its own data.

## Konklav

r[auth.relying-system]
Kuria MUST verify every request's token, its session and any DPoP proof
through Konklav's relying SDK. It MUST NOT verify tokens, or decide access to
an endpoint, in code of its own.

r[auth.service-principal]
Kuria MUST follow Konklav's change feed as a service principal, with a secret
key it is configured with, and MUST start doing so before it serves requests,
so that revoked sessions and changed keys take effect without a restart.

## Requests

r[auth.graphql.unauthenticated]
A request to the GraphQL endpoint without a token, or with a token that is
invalid, expired or whose session has ended, MUST be refused with 401 and a
`WWW-Authenticate` challenge, before any resolver runs.

r[auth.graphql.tenant-token]
The GraphQL endpoint MUST accept only tenant tokens, since everything in
Kuria belongs to a tenant. A request with any other kind of token MUST be
refused with 403.

r[auth.tenant]
The tenant a request acts in MUST be the tenant of its verified token. Kuria
MUST NOT take it from the query, its variables or any header.

r[auth.dpop]
Kuria MUST check DPoP proofs against the URL the client addressed, built from
the public URL Kuria is configured with, so that sender-constrained tokens
work behind a proxy or a load balancer.

r[auth.unavailable]
When Konklav cannot be reached and the relying SDK cannot trust its caches,
Kuria MUST refuse requests with 503 rather than admit them.

r[auth.viewer]
The GraphQL query `viewer` MUST return the subject and the tenant of the
request's verified token.

## Health

r[auth.health]
`GET /healthz` MUST answer without a token, and MUST NOT reveal anything
beyond the fact that the server is running.
