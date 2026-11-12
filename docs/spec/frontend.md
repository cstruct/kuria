# Frontend

Kuria's app is a single-page app served from the server's own origin. It
talks to the server only through GraphQL and the attachment endpoints, and
knows no akt type: every view is built from the akt types it reads.

## Serving

r[app.serve]
When given a directory of the built app, the server MUST serve its files,
and MUST answer every other `GET` that no route of its own takes with the
app's fallback page, so that the app's own paths load it. Hashed assets
MUST be cacheable for a year; the fallback page MUST NOT be cached.

r[app.csp]
The app MUST be served with a Content Security Policy that allows scripts,
styles, images and connections from Kuria's own origin only, and forbids
framing.

## Signing in

r[app.sign-in]
When the server answers that the request isn't signed in, the app MUST
send the browser to `/auth/login`, returning to the page it was on.

## Views

r[app.akt-types]
The app MUST build its forms, akt pages, boards and lists from the akt
types the server serves, and MUST NOT name any akt type, field, state or
transition of its own.

r[app.fields]
The app MUST show and edit each field by its kind: text in a single-line or
multiline box, a choice or choices from the field's options, a date, a user
or contact chosen by name, a checkbox that can be checked for someone else,
a reference's URL and label, a git repository with a SHA or tag, a Docker
repository with a digest and tag, and an attachment uploaded or
downloaded.

r[app.transitions]
The app MUST offer the transitions an akt's state allows, asking for the
fields a transition requires before performing it.

r[app.stale]
When an edit, transition or draft is refused as `STALE`, the app MUST show
the akt as it now is and keep what the person entered, so they can apply
it again.

r[app.errors]
The app MUST show every refusal the server answers with a message for its
code, and name the field a refused value belongs to.

r[app.live]
The app MUST follow the `changes` subscription while it is open, and show
an akt changed elsewhere without the person reloading.

r[app.accessible]
Every control MUST have an accessible role and name.
