# Boards

A board shows a container's members as columns. Members of different akt
types, such as stories and bugs in one epic, have different states, so the
container's akt type says how they line up.

r[board.columns]
An akt type document MAY configure `board.columns`. Each column MUST have a
unique key other than `unmapped` and a name, and MAY name, per akt type, the
states of that akt type that fall in it. Every akt type a column names MUST
be one the akt type contains, and a state of one akt type MUST NOT fall in
two columns.

r[board.placement]
A container's board MUST show each member it may be shown to in the column
that names the member's akt type and current state, in rank order, and the
columns in the order configured.

Which states a column may name is checked against the akt types it names
whenever either is defined (`akt.type.state-references`), so a renamed state
can't silently empty a column.

r[board.unmapped]
A member whose akt type and state no column names MUST be shown in an extra
column, `unmapped`, after the configured ones, rather than left out.

r[board.visibility]
A board MUST leave out the members the subject may not read, as any list
of akter does.

r[board.default]
A container whose akt type configures no columns MUST be shown with all its
members in the `unmapped` column.

## Parent boards

A board can feed another: a release candidate's board is planned from a
backlog, and work created on it belongs in that backlog too. Which board
is the parent is chosen per akt, as a link.

r[board.parent]
An akt type document MAY name, as `board.parent`, a link kind it declares
(`link.kinds`). The akter an akt links to by that kind are its board's
parent boards. A `board.parent` that names no link kind the document
declares MUST be refused.
