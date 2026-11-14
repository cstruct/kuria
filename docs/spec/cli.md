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
- `schema <type>`: a type's fields with their kinds, whether required, and
  options; its states and their tones; its transitions with what they
  require; the types it contains and links to; its board and presentation;
- `ls [<type>]`: akter as a table of number, title, state and the
  presentation's list fields, filtered by `--state`, `--in <container>`,
  `--where <field>=<value>` (`akt.filter`, `@me` for oneself) and
  `--search`, a page at a time, or every page with `--all` (`akt.page`);
- `find <text>`: a search (`akt.search`);
- `cat <number>…`: each akt as an akt file (`cli.file`);
- `show <number>`: an akt's fields, containers, members by column and
  type, links, and the transitions its state allows, and with `--history`
  its revisions;
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

r[cli.apply]
`kuria apply <file>…` MUST edit each akt with the fields its file changed
since the revision the file names, and only those (`app.inline-edit`). It
MUST refuse:
- a change to a read-only key, or to a key the type lacks, saying which;
- a file whose revision is stale, as `STALE`, saying which fields changed
  since, so the file can be merged and applied again.

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
- `add <container> <member>…` and `remove <container> <member>…`;
- `transfer <member> <from> <to>`;
- `move <number> --before <number>` or `--after <number>`;
- `link <source> <kind> <target> [--pin]` and `unlink <source> <kind>
  <target>`;
- `draft <number> <field>`;
- `types export [<key>…]` and `types define <file>…`, the latter with
  `--check` to validate documents without defining them;
- `export (--types <key>,… [--akter] | --whole-tenant) -o <dir>` and
  `import <dir> [--map-subjects <file>]` (`bundle.format`).

Every command that edits, transitions, links or arranges MUST take
`--reason`, recorded with the revisions it makes.

r[cli.values]
`--set` and `--where` values MUST be read by the field's kind: an option by
key or name, a user or contact by email, name or `@me`, a date as
`YYYY-MM-DD`, a checkbox as `true`, `false` or `for:<someone>`, and a
reference, Git or Docker value as JSON. A multiline value MAY be given as
`@<file>` or `@-` for standard input.

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
front-matter key, `rank`, its place in the directory's order. Any nested
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
pattern; the directory keeps no counter.

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

r[cli.local.unsupported]
In local mode `login`, `logout`, `users`, `contacts`, `draft`, attachments,
`export` and `import` of bundles MUST be refused as `LOCAL_UNSUPPORTED`,
saying why. `whoami` MUST answer the directory and who `@me` is.

r[cli.local.check]
`kuria check --dir <path>` MUST check the whole directory as a server
would: every document valid and each akt fitting its type, numbers
unique, every `in` and `links` entry naming an akt the directory has by a
kind its type declares, containment rules held, orders unique, each akt
stored once and in the file `cli.local.nesting` puts it in, and each file
named as `cli.local.names` says. It MUST name each problem's file, and the
akt's number for a nested akt, and exit with 1 if there is any, 0 if
none.

r[cli.local.convert]
`kuria export --types <key>,… --to-dir <path>`, signed in to a server, MUST
write the selected akt types and their akter as a new akt directory, in
the server's order, each akt reading through `cat` as it does on the
server, refusing an akt that doesn't fit its type's latest document, which
is the one an akt directory keeps, and one holding an attachment.
`kuria import --from-dir <path>` MUST import a directory `kuria check`
finds consistent as a bundle of its types and akter (`bundle.import.*`),
keeping their numbers, memberships, links and pins. Since git holds a
directory's history, each akt MUST get revisions numbered up to its file's
`revision`: the first recording it as it is, the rest changing nothing.
