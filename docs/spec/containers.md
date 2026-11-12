# Containers

An akt can contain other akter: a backlog contains backlog items and epics,
an epic contains backlog items, a release contains backlogs, release
candidates and hotfixes. Which akter may contain which is configuration on
their akt types. Being in a container is a membership, and every membership
that starts or ends is part of both akter's records.

## Configuration

r[container.contains]
An akt type document MAY list, in `contains`, the akt types whose akter its
akter may contain, each by key. An akt MUST only become a member of a
container whose akt type version lists the member's akt type.

r[container.exclusive]
An entry of `contains` MAY say `"exclusive": true`. An akt of that type
MUST then be a member of at most one container of the containing akt type
at a time.

r[container.created-in]
An akt type document MAY list, in `created_in`, the akt types an akt of its
type must be created in. Creating such an akt MUST name a container of one
of those types, which becomes the akt's parent. Each akt type in
`created_in` MUST be one whose akter can contain it, which is checked when
the akt is created.

## Memberships

r[container.many]
An akt MAY be a member of any number of containers at once, but of each
container only once.

r[container.add]
Adding an akt to a container MUST be refused when the container's akt type
doesn't contain the akt's type, when the akt already is a member, or when
it would break exclusivity, acyclicity or a frozen container. A container
and its members MUST be in the same tenant.

r[container.remove]
Removing an akt from a container MUST be refused when it isn't a member, or
when the container is frozen.

r[container.acyclic]
Containment MUST be acyclic: an akt MUST NOT become a member of itself, or of
any akt it contains, directly or through other containers.

r[container.frozen]
A container in a final state MUST NOT gain or lose members. Its members MAY
still be in any state, and MAY still join or leave other containers.

r[container.revisions]
Adding or removing a member MUST append one akt revision to the member and
one to the container, in the same transaction, each recording the other
akt, the subject who made the change and the reason if one was given, with
a change event for each. Neither akt's state or field values change, so
their state hashes stay the same.

r[container.unversioned]
Adding and removing members MUST NOT require the revision either akt was
based on, so that arranging a board isn't refused because someone edited a
member. Both akter MUST be locked while the change is made.

r[container.create-inside]
Creating an akt MAY name containers for it to be created in. It MUST then
be a member of each of them from its first revision on, which records those
memberships, and each container MUST accept it as `container.add` requires.
The first container named is its parent.
