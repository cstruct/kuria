# Command-line client

`kuria` is Kuria's command-line client, for people and for agents. It
reads akter like files and answers in Markdown or JSON, edits an akt as a
Markdown file, and changes state, membership, order and links through
explicit commands, since none of those is an edit. Like the app, it knows
no akt type. In local mode (`cli.local.dir`) it works the same way on a
directory of Markdown files instead of a Kuria server.

## Signing in

r[cli.profile]
The client MUST take Kuria's URL and the tenant from `--url` and
`--tenant`, else from `KURIA_URL` and `KURIA_TENANT`, else from the profile
`kuria login` saved, by `--profile` or the default one.

r[cli.login]
`kuria login` MUST sign in through the browser, by a link, as the public
Konklav client `kuria-cli`:

1. It asks Kuria where its Konklav is (`cli.discovery`), makes an ES256
   key for DPoP, a PKCE verifier and a random `state`,
   and prints the authorization URL, with the code challenge, the key's
   thumbprint as `dpop_jkt`, and the tenant asked for. It MAY also open
   the URL in a browser.
2. Once the person authorizes it, it collects the code from Kuria's relay
   (`cli.relay`) with the `state`.
3. It exchanges the code at Konklav's token endpoint with the verifier and
   a DPoP proof, and keeps the DPoP-bound session it gets.

The link MUST work when opened on another machine than the one the client
runs on.

r[cli.secret]
The DPoP key and the refresh token MUST be kept in the operating system's
credential store, or, where there is none, in a file only the user may
read. The client MUST NOT print them, or write them anywhere else. It MUST
refresh its access token before it expires, and `kuria logout` MUST revoke
the session at Konklav and forget the key and tokens.

r[cli.secret-key]
With `KURIA_KEY` set to a Konklav secret key, the client MUST sign in with
it instead, as software does: a DPoP-bound secret-key login at the issuer
`cli.discovery` names, then moving the session into the tenant. It MUST
NOT save that session, and SHOULD sign it out when the command is done.

r[cli.dpop]
Every request the client sends to Kuria or Konklav with its token MUST
carry a DPoP proof for that request.

## Kuria's side

r[cli.discovery]
Kuria MUST answer `GET /cli/config`, without authentication, with
`{"issuer", "client_id", "redirect_uri", "tenant"}`: its Konklav's issuer,
`kuria-cli`, its relay's URL, and the tenant a sign-in that names none is
for (`session.login.default-tenant`), or `null`. So `kuria login` needs
only Kuria's URL.

r[cli.relay]
Kuria MUST serve `/cli/authorized` as `kuria-cli`'s redirect URI: it keeps
the authorization code it is given, or the error, for the request's
`state`, for two minutes, and tells the person to return to their
terminal. `POST /cli/authorized/collect` with that `state` MUST answer the
code or the error once, and nothing for a `state` it doesn't hold. The
relay MUST NOT exchange or log codes. Collecting MUST be limited per client
address, as sign-in is.

## Answers

r[cli.format]
Every command that answers MUST answer in Markdown by default, readable as
the app shows the same things, and in JSON with `--format json`. JSON
answers MUST be objects whose members this spec and the client's guide
name, stable across versions of the same major version, not GraphQL's
answers passed through.

r[cli.errors]
A refused command MUST say the server's code and message, and the field it
belongs to if any; with `--format json`, as `{"error": {"code", "message",
"field"}}` on standard error. It MUST exit with 1 when refused for the
request's content, 2 for something not found, 3 for a stale revision, 4
when not signed in or not permitted, and 5 when Kuria or Konklav is
unavailable, and with 0 when done.

r[cli.guide]
`kuria guide` MUST print a guide for agents: the commands, the akt file,
the exit codes, and how to find an akt type's fields, states and
transitions.

## Reading

r[cli.read]
The client MUST offer:

- `types`: the akt types, with how many akter each has;
- `types schema <type>`: a type's fields with their kinds, whether required, and
  options; its states and their tones; its transitions with what they
  require and the outcomes they allow; the types it contains and links
  to; its board and presentation;
- `ls [<type>]`: akter as a table of number, title, state, outcome where
  they have one, and the presentation's list fields, filtered by
  `--state`, `--in <container>`, `--where <field>=<value>` (`akt.filter`,
  `@me` for oneself), `--outcome <outcome>` (`akt.filter.outcome`) and
  `--search`, a page at a time, or every page with `--all` (`akt.page`);
- `find <text>`: a search (`akt.search`);
- `cat <number>…`: each akt as an akt file (`cli.file`), several as one
  multi-akt file; with `--members <type>,…`, each followed by its direct
  members of those akt types, or with `--all-members` by all of them, in
  the container's order (`cli.members`);
- `show <number>`: an akt's fields, its outcome, containers, members by
  column and type, or in their sections with each section's outcome,
  links, its broken references (`akt.field.markdown.references`), its
  versions, and the transitions its state allows, and with
  `--history` its revisions;
- `cat`, `show` and `pdf` with `--version <n>`: the akt at that version
  (`version.read`);
- `versions <number>`: its versions, latest first (`version.list`), and
  `diff <number> <from> [<to>]`: what changed between two versions, or a
  version and the living version, `markdown` fields line by line
  (`version.diff`);
- `pdf <number> [--field <key>] [--version <n>] -o <file>`: a `markdown`
  field as a PDF (`pdf.route`), the type's only one if no field is named;
- `history <number>`: its revisions, as the app's history says them
  (`app.history`);
- `users` and `contacts`: whom values can name.

## The akt file

r[cli.file]
An akt file MUST be Markdown with YAML front matter:

- **front matter:** `number`, `type` and `revision` (the revision it was
  read at), then `state`, `in` (its containers) and `links`, which are
  read-only, then each single-line field by key;
- **body:** each multiline field as a section that starts with
  `<!-- field: <key> -->` and runs to the next such marker, in its type's
  field order. A marker inside a fenced code block is the field's text.

Users and contacts MUST be written by email, else name, and read back by
email, name or `@me`. A field without a value MUST be left out.
`cat` followed by `apply` of the unchanged file MUST change nothing.

r[cli.file.kinds]
In an akt file, a `multiple` `user` field MUST be a list of users and
contacts; a `sections` field MUST be a section of its own whose body is
the field's value as YAML, akter by number; and a `verdict` field MUST be
a front-matter mapping of each assignee to their verdict, read-only, given
with `verdict`. An akt that has outcomes MUST have a read-only `outcome`
key after `state`. A `markdown` field MUST be a section of its own whose
body is its value. `links` MUST leave out the links references keep
(`link.referenced`), to or from the akt: the text says them, and they
change without a revision of their target. An akt file MUST NOT list its versions, which change without a
revision. An akt read at a version MUST have a read-only `version` key
after `revision`, and `apply` of it MUST be refused as
`READ_ONLY`.

r[cli.file.multi]
A multi-akt file MUST hold its first akt as an akt file, and each other
after a line `<!-- akt: <NUMBER> -->`, as its akt file without `number`,
which the line gives; such a line inside a fenced code block is text. It
is an akt directory's file without `rank` (`cli.local.layout`). `cat` of
more than one akt MUST print one; `cat` of one akt prints its akt file.

r[cli.members]
`cat --members <type>,…` MUST answer each akt it is given followed by its
direct members of those akt types, and `--all-members` by all its direct
members, in the container's order, whatever their types; members are the
akter whose `in` lists it, on a server as in an akt directory. An unknown
akt type MUST be refused as `UNKNOWN_AKT_TYPE`. In JSON, `akter` MUST list
them in that order; with `-o <dir>`, each MUST be written to its own file.

r[cli.apply]
`kuria apply <file>…` MUST edit each akt with the fields its file changed
since the revision the file names, and only those (`app.inline-edit`),
each akt of a multi-akt file (`cli.file.multi`) as if it were a file of
its own, its number from its line or its `number`, which MUST agree. It
MUST read and check every akt it is given before changing any, and,
against a server, MUST then send every edit in one batch (`akt.batch`), so
that a refusal of any changes none. It MUST refuse, changing nothing:
- a change to a read-only key, or to a key the type lacks, saying which;
- an akt whose revision is stale, as `STALE`, saying which fields changed
  since, so the file can be merged and applied again;
- an akt given twice.

With `--dry-run` it MUST say what it would change and change nothing. `new
<type> --file <file>` MUST create an akt from a file, ignoring `number` and
`revision`.

## Writing

r[cli.write]
The client MUST offer:

- `new <type> [--in <container>]… [--set <field>=<value>]…`, answering the
  new akt's number;
- `set <number> <field>=<value>…`: an edit without a file;
- `do <number> <transition> [--set <field>=<value>]…`: a transition, with
  the fields it requires;
- `members add <container> <member>…` and `members remove <container>
  <member>…`;
- `members transfer <member> <from> <to>`;
- `members place <number> --before <number>` or `--after <number>`;
- `link <source> <kind> <target> [--pin]` and `unlink <source> <kind>
  <target>`;
- `draft <number> <field>`;
- `verdict <number> pass|fail|skip|clear [--for <user>]`: one's own
  verdict, or one for another assignee (`akt.field.verdict`);
- `copy <number> [--only <number>…] [--in <container>…]`, answering the
  copy's number and the numbers of the copies of its members (`akt.copy`);
- `release <number> [--label <label>] [--file <pdf>]`: its current
  revision as its next version, answering the version's number, or each
  broken reference (`version.release`, `version.references`); with
  `--file`, the PDF an akt type whose versions carry one is released with,
  uploaded first (`version.release-file`);
- `docs clone`, `docs status`, `docs diff`, `docs add`, `docs pull` and
  `docs push`: a container as files synced with Kuria (`clone.*`);
- `docs fmt`, in a clone: its files in normal form (`clone.normal`);
- `types export [<key>…]` and `types define <file>…`, the latter with
  `--check` to validate documents without defining them;
- `export (--types <key>,… [--akter] | --whole-tenant) -o <dir>` and
  `import <dir> [--map-subjects <file>]` (`bundle.format`);
- `archive <number>… [--reason]` and `unarchive <number>… [--reason]`
  (`cli.archive`).

Every command that edits, transitions, links or arranges MUST take
`--reason`, recorded with the revisions it makes.

r[cli.values]
`--set` and `--where` values MUST be read by the field's kind: an option by
key or name, a user or contact by email, name or `@me`, a date as
`YYYY-MM-DD`, a checkbox as `true`, `false` or `for:<someone>`, and a
reference, Git or Docker value as JSON. A multiline value MAY be given as
`@<file>` or `@-` for standard input.

r[cli.archive]
`kuria archive <number>… [--reason]` and `kuria unarchive <number>…
[--reason]` MUST archive and unarchive each akt (`archive.act`), in order,
stopping at the first refused, those before it staying so. `ls`, `find`
and `cat`/`show` of a container's members MUST answer live akter, or with
`--archived` only archived ones, or with `--with-archived` both (`--all`
of `ls` being every page); `show` of an archived container MUST list all
its members unless asked otherwise, as its board does (`archive.board`).
An archived akt's answer MUST say it is archived: in Markdown, `(archived)`
by its state in a list, beside its members' and containers' states, and
when, by whom or which rule and why in `show`; in its akt file, a
read-only `archived` key after `state` (`{at, by, rule, reason}`); in
JSON, an `archived` object with `at`, `by`, `rule` and `reason`, `null`
for a live akt.

## Local mode

In local mode the client reads and writes an akt directory instead of a
Kuria server: akt type documents and akt files on disk, with git keeping
their history. The commands, answers, refusals and exit codes are those
above.

r[cli.local.dir]
With `--dir <path>`, else `KURIA_DIR`, the client MUST work on the akt
directory at `path` instead of a server, whatever `--url`, `KURIA_URL` or
a profile say. `kuria init --dir <path>` MUST make one: `kuria.toml`
naming its format version, `types/` and `akter/`. Any other command MUST
refuse a directory without a `kuria.toml` whose format it reads, as
`NOT_AN_AKT_DIRECTORY`.

r[cli.local.layout]
An akt directory MUST hold each akt type's document as
`types/<key>.json`, one version per key, and its akter in `akter/*.md`.
Each file MUST begin with an akt as an akt file (`cli.file`) with one more
front-matter key, `rank`, its place in the directory's order, and without
`outcome`, which is derived each time it is read (`akt.outcome.derived`). Any nested
akter (`cli.local.nesting`) MUST follow it, each beginning with the line
`<!-- akt: <NUMBER> -->` and then as an akt file of its own with `rank`
but without `number`, which the line gives. Such a line inside a fenced
code block MUST be read as text. An akt MUST be found by its number,
whatever file holds it. Files MUST be written whole, so that a reader
never sees half a write.

r[cli.local.nesting]
An akt whose number its type's pattern makes from its parent's
(`{parent}`) MUST be stored in the file that holds that parent, as a nested
akt; any other akt MUST begin a file of its own. A file's akter after the
first MUST be in the directory's order, each after its parent. Which file
holds an akt MUST NOT say what it is in: its `in` lists every container,
its parent too, as its akt file does, so that one removed from its parent
stays in the parent's file and in nothing. A change MUST write each file
whose akter it touched, whole.

r[cli.local.names]
A file MUST be named by the akt it begins with: `<NUMBER>-<slug>.md`,
where the slug is its title in lower case with every run of characters
other than ASCII letters and digits made one `-`, trimmed of `-` and cut
at a `-` to at most 40 characters; or `<NUMBER>.md` if that leaves
nothing. A change to that akt's title MUST rename the file in the same
change.

r[cli.local.types]
`types define` in local mode MUST replace the type's document, refusing a
document that the directory's akter of that type wouldn't fit (a state,
field or option they use that it lacks), naming them. `types export` MUST
answer the documents as they are.

r[cli.local.rules]
Writes in local mode MUST be checked by the same rules a server checks,
through the same code (`kuria-engine`): field values, transitions and what
they require, numbering, containment (`contains`, exclusive, `while`,
`created_in`, cycles, frozen containers), link kinds and pins, and
ordering. A refusal MUST carry the code a server would answer.

r[cli.local.revision]
Each write to an akt MUST raise its file's `revision` by one, and add or
remove it from the containers and links of the other akter it names, on
their files too. `apply` of a file read at an older `revision` MUST be
refused as `STALE` (`cli.apply`).

r[cli.local.numbers]
A new akt's number MUST be one past the highest sequence of its number's
scope among the directory's akt files, read through their type's number
pattern; the directory keeps no counter. A number without a sequence MUST
be drawn free of every number in the directory, archived akter's included
(`akt.number.random`).

r[cli.local.people]
Users and contacts MUST be written and read as text: an email, else a
name. `@me` MUST be `KURIA_ME`, else the email `git config user.email`
gives in the directory, and MUST otherwise be refused as `BAD_REQUEST`.

r[cli.local.history]
`history` in local mode MUST answer the commits that changed the akt,
newest last, each with its author, time, message and the akt's revision
after it, from git, and say when the akt has changes not yet committed. A
commit changed an akt when its `revision` changed, whichever file held it,
so that an akt moved between files keeps its history. When an akt was
created and last changed MUST be its first and last such commits, or,
with changes not committed, when its file was written. In a directory git
doesn't track, it MUST say there is no history, and every akt of a file
MUST be as old as the file.

r[cli.local.reason]
`--reason` in local mode MUST be accepted and kept nowhere: the answer
MUST give it back, as `reason` in JSON and as the message to commit with
in Markdown, since git is the history.

r[cli.local.lock]
A write in local mode MUST hold an exclusive lock on the directory for the
whole change, so that two clients writing at once neither lose a change
nor read one half made.

r[cli.local.references]
In local mode the links references keep (`link.referenced`) MUST NOT be
stored in akt files: they MUST be worked out from the directory's
`markdown` fields whenever they are read, as a server keeps them.

r[cli.local.archive]
In local mode archiving an akt MUST remove its entry from its file, and
its members' entries nested in it, so that git's history keeps them, and
MUST add its number to `akter/archived.md`, with when, by whom or which
rule, why, and the revision. A number there MUST NOT be reused, a
reference to it MUST resolve as archived, and `kuria unarchive` MUST put
the akt back as git's latest commit holding that revision had it, refused
as `UNCOMMITTED` while the directory has changes git hasn't committed.
Rules MUST apply on the changes the CLI makes (`archive.rules.on-change`),
and `kuria check --fix` MUST apply them all (`archive.sweep`).

r[cli.local.archive.kept]
`akter/archived.md` MUST hold its list as YAML front matter: `archived`,
each entry's `number`, `type`, `revision`, `at`, `by` or `rule` (with the
`container` a `with_container` archive went with) and `reason`, and
`kept`, the live akter a person unarchived after a rule archived them
(`archive.rules.kept`). The akter nested in an archived akt's file MUST
be archived with it, as `with_container` archives them, whatever their
types' rules. An archived akt that git doesn't hold at its revision as it
is, such as one with changes not committed yet, MUST also be kept as its
own akt file `akter/archived/<NUMBER>.md`, which `kuria unarchive` MUST
read before git and remove. Reading an archived akt MUST answer it at the
revision its archiving reached, one past the one listed, and unarchiving
it MUST raise it by one more, as a server's archive and unarchive
revisions do (`archive.history`). `kuria check` MUST name an entry that
is also live, of a type `types/` lacks, a kept akt that isn't live, and a
file in `akter/archived/` the list doesn't name.

r[cli.local.versions]
In local mode an akt's versions MUST be kept in the directory's file, as
a front-matter `versions` list of what `version.number` and
`version.pins` record, which, like `rank`, `cat` leaves out: a release
makes no revision, so an akt file can't change with one. A release MUST be refused as `UNCOMMITTED` while
the akt has changes git hasn't committed, and reading an akt at a version
MUST read it from the latest commit at which it had the version's
revision (`cli.local.history`). A release of an akt whose type declares `versions.file`
MUST copy the file in as `cli.local.attachments` says and record it as a
new revision (`version.release-file`), which the version names.

r[cli.local.attachments]
In an akt directory, an attachment MUST be a file under
`attachments/<number>/<name>`, which the akt file's value for an
`attachment` field names by that path from the directory. Local mode MUST
check that the file exists, that its media type, by its extension, is one
its field takes (`attachment.media-types`), and that a field holding SBOMs
holds one (`attachment.sbom`), and MUST read pictures from those files
(`akt.field.markdown.images`). A value naming anything else MUST be
refused.

r[cli.local.unsupported]
In local mode `login`, `logout`, `users`, `contacts`, `draft`, uploading
attachments, `pdf`, `export` and `import` of bundles MUST be refused as
`LOCAL_UNSUPPORTED`, saying why. `whoami` MUST answer the directory and who `@me` is.

r[cli.local.check]
`kuria check --dir <path>` MUST check the whole directory as a server
would: every document valid and each akt fitting its type, numbers
unique, every `in` and `links` entry naming an akt the directory has by a
kind its type declares, containment rules held, orders unique, each akt
stored once and in the file `cli.local.nesting` puts it in, and each file
named as `cli.local.names` says. It MUST name each problem's file, and the
akt's number for a nested akt, and exit with 1 if there is any, 0 if
none. `kuria check --fix` MUST first move every akt into the file
`cli.local.nesting` puts it in and name every file, as a write would,
changing no akt, under the directory's lock, then check.

r[cli.local.convert]
`kuria export --types <key>,… --to-dir <path>`, signed in to a server, MUST
write the selected akt types and their akter as a new akt directory, in
the server's order, each akt reading through `cat` as it does on the
server, refusing an akt that doesn't fit its type's latest document, which
is the one an akt directory keeps. Each attachment MUST be written as
its file (`cli.local.attachments`). Each archived akt MUST be listed in
`akter/archived.md` and written to `akter/archived/` at the revision
before its archive's (`cli.local.archive.kept`), since no git history
holds it yet; an import MUST carry each archived akt, with its archive's
revision, and each kept one, as archived and kept (`bundle.contents`). Each
akt's versions MUST be written into its directory file's `versions`
(`cli.local.versions`), each with the headings it had at its revision; a
version's text stays with the server, since git holds none of the
revisions before the directory's.
`kuria import --from-dir <path>` MUST import a directory `kuria check`
finds consistent as a bundle of its types and akter (`bundle.import.*`),
keeping their numbers, memberships, links, pins, versions and
attachments, each uploaded from its file. Since git
holds a directory's history, each akt MUST get revisions numbered up to its
file's `revision`, each recording the akt as git's latest commit at that
revision holds it, else as at the next revision git has, and its file as it
is for its own; the first records it whole, and each later one what
changed. An import MUST be refused as `NOT_FOUND` when git has no commit at
a revision one of its versions names.
