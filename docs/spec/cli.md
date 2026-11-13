# Command-line client

`kuria` is Kuria's command-line client, for people and for agents. It
reads akter like files and answers in Markdown or JSON, edits an akt as a
Markdown file, and changes state, membership, order and links through
explicit commands, since none of those is an edit. Like the app, it knows
no akt type.

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
  field order.

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
