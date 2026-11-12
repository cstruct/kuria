# Authorization

Konklav decides what a subject may do; Kuria asks it, operation by
operation, and enforces the answer. Kuria's actions and its scope kind are
catalog data in Konklav, so an operator grants them with Konklav roles:
tenant-wide, or at one akt type.

## Actions

r[authz.catalog]
Kuria MUST ask for these actions, which an operator declares in Konklav's
catalog, and for no others except those akt types name:

| Action | Required |
|---|---|
| `kuria:akt_types:define` | tenant-wide, to define akt types |
| `kuria:akter:read` | at the akt type, to see its akter |
| `kuria:akter:create` | at the akt type, to create its akter |
| `kuria:akter:edit` | at the akt type, to edit its akter |
| `kuria:akter:transition` | at the akt type, to perform its transitions |

An action required at an akt type MUST be required at the scope of kind
`kuria_akt_type` whose id is the akt type's key, so that either a
tenant-wide grant or a grant at that akt type satisfies it.

r[authz.transition-action]
A transition MAY name one more action, such as `capa:approve_closure`, in
its `action`. Performing it MUST then also require that action at the akt
type. The action MUST be colon-delimited and MUST NOT be in the `konklav:`
namespace; otherwise the akt type document is refused.

## Decisions

r[authz.decide]
Every permission MUST be decided by the relying SDK for the request's token.
One request MAY reuse a decision for the same requirements; nothing else
MAY. When the relying SDK cannot decide, the operation MUST be refused with
the code `UNAVAILABLE`, never allowed.

r[authz.sender-constrained]
Since a sender-constrained token's proof is answered once, Kuria cannot
decide more than once for it. The GraphQL endpoint MUST refuse requests with
sender-constrained (DPoP) tokens with 403 and the code
`sender_constrained_unsupported`.

## Operations

r[authz.define]
Defining an akt type MUST require `kuria:akt_types:define`, and be refused
with the code `FORBIDDEN` otherwise.

r[authz.read-types]
An akt type MUST be visible only to subjects who may define akt types or
read its akter. To anyone else it MUST be as if it did not exist.

r[authz.read]
An akt and its revisions MUST be visible only to subjects who may read the
akter of its akt type. To anyone else, queries MUST behave as if it did not
exist, lists MUST leave it out, and mutations naming it MUST be refused as
`NOT_FOUND`.

r[authz.create]
Creating an akt MUST require `kuria:akter:create` at its akt type. A subject
who can't see the akt type MUST be refused as `UNKNOWN_AKT_TYPE`; one who
can, as `FORBIDDEN`.

r[authz.edit]
Editing an akt MUST require `kuria:akter:edit` at its akt type, and be
refused as `FORBIDDEN` otherwise.

r[authz.transition]
Performing a transition MUST require `kuria:akter:transition` at its akt
type, and the transition's own action if it names one, and be refused as
`FORBIDDEN` otherwise.
