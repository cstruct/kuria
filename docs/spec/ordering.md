# Ordering

Every akt has one rank: its place in its tenant's one priority order. Every
board orders its members by rank, so a move made on one board shows on
every board.

r[order.rank]
Every akt MUST have a rank that is unique in its tenant. A new akt MUST be
ranked after every other akt of its tenant.

r[order.move]
Moving an akt before or after another akt of its tenant MUST give it a rank
directly before or after that akt, so that no other akt of the tenant ranks
between them. Moving an akt relative to itself MUST be refused.

r[order.everywhere]
Wherever akter are listed by rank, as members of a container or on a board,
they MUST be in the order of their ranks, so that two akter keep the same
relative order on every board that shows both.

r[order.not-recorded]
A move MUST NOT append an akt revision. It MUST append a change event
naming the akt, so that boards showing it can update.

r[order.concurrent]
Moves made at the same time MUST NOT leave two akter with the same rank,
and each MUST either take effect or be refused.
