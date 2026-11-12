# Drafts

A field can be drafted from the akter around it: release notes from the
items a release's candidates and hotfixes contain, say. A draft is an
ordinary edit of the field, so it is recorded, reviewed and changed by hand
like any text.

## Recipes

r[draft.recipe]
A multiline text field MAY declare a recipe in `draft`:

| Member | Meaning |
|---|---|
| `through` | The akt types of members to look further inside, by key; any if left out. |
| `depth` | How many levels of members to look through, from 1 to 5; 1 if left out. |
| `types` | The akt types to list, by key; at least one. |
| `line` | The text of each listed akt, with placeholders. |
| `group_by` | `type` to list akter under a heading per akt type; one list if left out. |

A recipe on any other field, or with an invalid key, depth or line, MUST be
refused with the akt type document.

r[draft.line]
A line MUST be literal text with these placeholders: `{number}`, `{type}`
(the akt type's name), `{state}` (the state's name) and `{field:<key>}`
(that field's value as text). Literal text MUST NOT contain `{` or `}`.

## Drafting

r[draft.sources]
Drafting a field MUST look inside the containers the akt is in, then
through their members and their members' members down to `depth` levels,
looking further inside only members whose akt types `through` lists, and
list every akt found whose akt type `types` lists, once, without the akt
itself. It MUST leave out, and not look inside, every akt the drafting
subject may not read.

r[draft.render]
Each listed akt MUST give one line, its placeholders replaced: a field's
value as its text, the name of its option or options (joined with `, `),
its date as `YYYY-MM-DD`, its subject's identifier, or its reference's
label or else its URL; and an empty text for a field the akt doesn't have
or that holds no value. With `group_by` `type`, the lines MUST be grouped
under a heading `## <akt type name>` per akt type, in the order `types`
lists them, leaving out empty groups; otherwise they form one list. Lines
MUST be in rank order.

r[draft.edit]
Drafting a field MUST be an edit of the akt that sets the field to the
draft, with everything an edit requires: the revision it is based on,
`kuria:akter:edit`, and an akt not in a final state. It MUST be refused
for a field without a recipe.
