# Links

A link is a typed, directed relationship from one akt to another, such as
a task that blocks another, or a CAPA and the complaint it originated
from. Containment is the one link kind Kuria builds in
([containers](containers.md)); every other kind is declared by the akt
types that use it.

## Link kinds

r[link.kinds]
An akt type document MAY declare, in `links`, the kinds of link its akter
may have to other akter. Each has a key, a name for the link as its
source reads it ("Blocks"), an `inverse` name as its target reads it
("Blocked by"), and, in `to`, the akt types its targets may have, by key.
Keys MUST follow `akt.type.validation` and be unique within the document,
names and inverse names MUST NOT be empty, and `to` MUST name at least one
akt type, each by a key that follows `akt.type.validation`, once;
otherwise the document MUST be refused. As with `contains`, the types `to`
names need not be defined yet, so akt types can be defined in any order.

## Linking

r[link.add]
`addLink` MUST link an akt to another by a kind its akt type version
declares, when the target's akt type is one the kind's `to` names. It MUST
refuse a link to the akt itself, and a link of the same kind between the
same two akter that already exists, as `INVALID_LINK`.

r[link.create]
`createAkt` MAY name links for the new akt, each by a kind and another
akt: `to` it, with the new akt as the link's source, or `from` it, as its
target. Each MUST be added as `addLink` adds it (`link.add`,
`authz.link`), in the transaction that creates the akt, so that it never
exists without them; a refused link refuses the creation.

r[link.create.carry]
Creating an akt linked to another MAY carry over that akt's values: each
field both akt types have by key and kind starts with the other akt's
value, which a value given for it replaces. The app MUST offer, on an
akt's page, for each link kind by which it can link to, or be linked from,
an akt type the viewer may create, a *New …* action: the type's create
page, asking for the carried values to keep or change, then creating it
linked (`app.create`). The CLI does so with `kuria new <type> --link
<kind>:<number>` for a link from the new akt, `--linked-from
<kind>:<number>` for one to it, and `--carry` (`cli.write`).

r[link.referenced]
A link kind a `markdown` field names as its `references`
(`akt.field.markdown.references`) MUST be kept by the akt's writes alone:
`addLink` and `removeLink` MUST refuse it as `INVALID_LINK`. Its links
changing MUST be recorded in the source's revision that changed them, and
MUST NOT make a revision of the target, which may be final.

r[link.pin]
A link MAY pin a revision of its target. It MUST then be shown with the
target's state and title as they were at that revision, and the revision's
number, while still naming the target.

r[link.remove]
`removeLink` MUST end a link of one kind from one akt to another, and MUST
refuse one that doesn't exist as `NOT_FOUND`.

r[link.revisions]
Adding or removing a link, other than one a reference keeps
(`link.referenced`), MUST be recorded as one akt revision of each of
the two akter, as membership is (`container.revisions`), saying the kind,
the other akt by number, and any pinned revision. It MUST NOT be refused
as stale: like arranging members, it takes no revision from the caller.

## Reading links

r[link.read]
An akt MUST be served with its links: those from it, by kind, and those to
it, by the kind's inverse name, each with the other akt, and whether a
reference keeps it (`link.referenced`). A link to or from
an akt the subject may not read MUST be left out.
