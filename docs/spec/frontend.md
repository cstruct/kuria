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
repository with a digest and tag, an attachment uploaded or downloaded,
several users or contacts for a `multiple` field, each assignee's
verdict, and a `markdown` field in a rich-text editor (`app.editor`).

r[app.inline-edit]
The app MUST let a person edit one field of an akt at a time, in place,
saving only that field.

r[app.markdown]
The app MUST render multiline text as Markdown, as sanitized HTML: no
scripts, styles, frames, forms or event handlers, and links only to
`http`, `https` and `mailto` addresses, opened without the opener.

r[app.editor]
The app MUST edit a `markdown` field in a rich-text editor offering
headings, paragraphs, emphasis, strikethrough, lists, task lists, quotes,
code, tables, horizontal rules, links, references, footnotes of one
paragraph each, embedded images and, in a field that allows them, diagram
blocks, and nothing the field's Markdown can't hold. Inserting an image
MUST let a person find an image akt (`app.pickers`), or upload a file,
which creates an image akt of an image type they choose, at the top
level, titled by the file's name. A diagram block MUST be shown as its
picture, rendered without being stored (`diagram.preview`), or as its
code, as the person toggles it, showing typst's message while it doesn't
compile. Inserting a
footnote MUST put the next unused number at the cursor and its
definition at the end, starting plain, where typing goes on. Saving MUST give the field as Markdown, and
opening and saving an unchanged value MUST change nothing, its heading ids
included. Inserting a reference MUST let a person find an akt
(`app.pickers`), then choose one of its headings or none, and its living
version or one of its versions.

r[app.references]
The app MUST render a `markdown` field's headings with their ids as
anchors, and each reference as a link to the akt's page, at its version if
it names one or a version read pins it (`version.read`), and to its
heading if it names one. A broken reference MUST be shown as broken,
saying why. Each reference to a footnote MUST be its number, in the order
first referred to, linking to the note, and the notes MUST be listed after
the text, each linking back; a definition nothing refers to MUST NOT be
shown.

r[app.versions]
An akt's page MUST list its versions, if its type declares them, and let a
person release its current revision with an optional label, showing each
broken reference a refusal names, and offering to release it together
with each akt the refusal names under `together` (`version.together`),
chosen by default. It MUST let a person read the akt at a
version, saying which and that it is not the living version, and compare
two versions, or a version and the living version (`version.diff`).

r[app.pdf]
The app MUST offer a `markdown` field as a PDF, of the living version or
of the version being read (`pdf.route`), to download, and each version's
PDF opening in a new tab from the versions list.

r[app.sheets]
The app MUST show and edit a `markdown` field as A4 sheets, white with
dark text whatever the theme, with the PDF's margins, starting a new sheet
at each page break, and marking where a sheet would otherwise end. The
editor MUST offer inserting a page break.

r[app.live]
The app MUST show an akt with a live draft as its live draft, and edit it
together with everyone else editing it (`live.sync`): each person's edits
appearing to the others as they make them, without saving, and who else
is editing, and where, shown (`live.presence`). Releasing it MUST release
the live draft as it is (`live.release`).

r[app.transitions]
The app MUST offer the transitions an akt's state allows, asking for the
fields a transition requires before performing it. A transition the akt's
outcome doesn't allow (`akt.transition.outcomes`) MUST be shown as
unavailable, saying the outcomes it allows.

r[app.members]
A container's page MUST show its members of the akt types its board names
on the board, and its members of every other akt type as a list per akt
type, not as one unmapped column. Cards and lists MUST show each member's
number, title, state and the fields its type's presentation names for
them.

r[app.sections]
An akt with a `sections` field MUST show its members in those sections,
nested as the field nests them: each section with its title, if any, its
text as Markdown, and its outcome, and one that is `collapsible` closed
until opened. Members no section names MUST follow, as `app.members`
shows them.

r[app.verdicts]
Each member with a `verdict` field MUST show, where its sections or its
container list it, its assignees, each one's verdict, and its outcome, and
MUST let an assignee give, change or take back their own verdict there.
An akt's page MUST show its outcome, if it has outcomes (`akt.outcome`).

r[app.copy]
The app MUST let a person copy an akt (`akt.copy`), choosing the sections
and members to keep, all of them at first, and showing the copy once it is
made.

r[app.board-drop]
Dropping a card on another column of a board MUST offer the transitions
from the member's state to a state that column names for its akt type,
and perform the one chosen as `app.transitions` does, asking for the fields
it requires. A column no such transition leads to MUST refuse the drop.

r[app.navigation]
The sidebar MUST list the akt types whose `navigation` is `more`
(`akt.type.presentation`) after the others, under a "Show more"
disclosure saying how many there are, closed until opened, remembering in
the browser whether it was. Every other place that lists akt types MUST
list them as before.

r[app.images]
The app MUST show an embedded image (`akt.field.markdown.images`) as its
picture, with its alt text as the picture's text, and one alone in its
paragraph as a figure captioned by it; a broken one, or an image of
anything but an akt, MUST be shown as its alt text, saying why. A picture
MUST only ever be an image of an attachment's download, never markup put
in the page, so that an SVG's scripts never run. An attachment of an
image's media type MUST be shown as its picture on an akt's page, on
cards and in lists.

r[app.diagrams]
The app MUST show a diagram block (`akt.field.markdown.diagrams`) as the
picture of its rendering, its source as the picture's text.

r[app.type-identity]
Wherever the app names an akt type, in the sidebar, on Home, in an akt's
header and on its type's list, it MUST show the type's icon and colour
(`akt.type.presentation`), with its name.

r[app.links]
An akt's page MUST show its links (`link.read`), from it by kind and to it
by inverse name, each with the other akt's number, title and state, and
let a person add a link of a kind its type declares, to an akt found by
number or title (`app.pickers`), and remove one.

r[app.filters]
A type's list MUST let a person filter it by the values of its filterable
fields (`akt.filter`), and by outcome where its type has a `verdict` field
(`akt.filter.outcome`), keeping the filters in the page's address, and load
more akter a page at a time (`akt.page`).

r[app.my-work]
Home MUST show the akter whose `user` fields, also those naming several
people, or checkboxes name the viewer, by akt type, for the types that have
such fields.

r[app.pickers]
Wherever the app asks for an akt, such as a container to add an akt to, or
a member to add, it MUST let the person find it by number or title
(`akt.search`), rather than only type its number.

r[app.search]
The app MUST offer search (`akt.search`) on every page, from the keyboard
too, and open the akt chosen.

r[app.history]
An akt's history MUST say, for each revision, who made it and when, its
transition, the fields it changed with their new values, the users and
contacts those values name by their names (`contact.show`), and the
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
