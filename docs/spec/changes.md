# Changes

Every change to an akt is a change event in Postgres (`akt.change-event`).
Kuria publishes them through Valkey so every server instance learns of
them, and delivers them to clients as a GraphQL subscription, after first
catching a client up on what it missed.

## Publishing

r[changes.publish]
Every change event MUST be published to its tenant's stream in Valkey, at
least once, after the transaction that wrote it commits, and MUST then be
marked published. An event an instance failed to publish MUST be published
later by any instance, so that none is lost when an instance stops between
commit and publish.

r[changes.identifiers]
What Kuria publishes to Valkey MUST name only the tenant and the change
event's sequence number, never field values or anything else about an akt.

## Delivery

r[changes.subscribe]
The subscription `changes` MUST deliver the change events of the
subscriber's tenant, each once, in sequence order, with the akt's number,
the kind of change and, for a revision, its number.

r[changes.visibility]
A subscription MUST leave out the change events of akter the subscriber may
not read, decided as for any query.

r[changes.catch-up]
A subscription MAY name the last sequence number its client saw, in
`after`. It MUST then first deliver every change event after it, read from
Postgres, and then continue with new ones, with no event missed or
delivered twice in between. Without `after`, it MUST deliver only change
events made after it started.

r[changes.lagging]
A subscriber that falls behind the events arriving MUST be caught up from
Postgres rather than miss any.

## Connections

r[changes.websocket]
Subscriptions MUST be served over WebSocket, with the `graphql-transport-ws`
protocol, at `/graphql/ws`, and the upgrade authenticated as `/graphql` is:
by a tenant token, or a session cookie from an allowed origin.

r[changes.expiry]
A subscription MUST end when the access token its connection was
authenticated with expires, so that a connection never outlives what
authorized it. The client reconnects, with `after`, to continue.
