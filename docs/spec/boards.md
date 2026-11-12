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

r[board.unmapped]
A member whose akt type and state no column names MUST be shown in an extra
column, `unmapped`, after the configured ones, rather than left out.

r[board.visibility]
A board MUST leave out the members the subject may not read, as any list
of akter does.

r[board.default]
A container whose akt type configures no columns MUST be shown with all its
members in the `unmapped` column.
