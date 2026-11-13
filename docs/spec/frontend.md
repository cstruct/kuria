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
framing. Inline, it MAY allow only the app's own bootstrap script and the
router's hidden announcer's style, each by its hash.

## Signing in

r[app.sign-in]
When the server answers that the request isn't signed in, the app MUST
send the browser to `/auth/login`, returning to the page it was on.

## Views

r[app.akt-types]
The app MUST build its forms, akt pages, boards and lists from the akt
types the server serves, and MUST NOT name any akt type, field, state or
transition of its own.

r[app.home]
The home page MUST show the akter the person can read that changed most
recently (`akt.list`), and each akt type with how many akter it has and a
way to create one.

r[app.header]
An akt's page MUST show first its akt type, number, title and state, the
state in its tone (`akt.type.presentation`), and the transitions its state
allows as actions there.

r[app.fields]
The app MUST show and edit each field by its kind: text in a single-line or
multiline box, a choice or choices from the field's options, a date, a user
or contact chosen by name, a checkbox that can be checked for someone else,
a reference's URL and label, a git repository with a SHA or tag, a Docker
repository with a digest and tag, and an attachment uploaded or
downloaded.

r[app.inline-edit]
The app MUST let a person edit one field of an akt at a time, in place,
saving only that field.

r[app.markdown]
The app MUST render multiline text as Markdown, as sanitized HTML: no
scripts, styles, frames, forms or event handlers, and links only to
`http`, `https` and `mailto` addresses, opened without the opener.

r[app.transitions]
The app MUST offer the transitions an akt's state allows, asking for the
fields a transition requires before performing it.

r[app.members]
A container's page MUST show its members of the akt types its board names
on the board, and its members of every other akt type as a list per akt
type, not as one unmapped column. Cards and lists MUST show each member's
number, title, state and the fields its type's presentation names for
them.

r[app.pickers]
Wherever the app asks for an akt, such as a container to add an akt to, or
a member to add, it MUST let the person find it by number or title
(`akt.search`), rather than only type its number.

r[app.search]
The app MUST offer search (`akt.search`) on every page, from the keyboard
too, and open the akt chosen.

r[app.history]
An akt's history MUST say, for each revision, who made it and when, its
transition, the fields it changed with their new values, and the
containers or members it added or removed, by number.

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

r[app.themes]
The app MUST follow the system's light or dark appearance, with text and
controls contrasting with their background as WCAG 2.2 level AA requires.

r[app.narrow]
Every page MUST be usable at 390 pixels wide, without scrolling sideways
except inside boards and tables.
