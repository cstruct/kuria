-- A repository's refs (`git.repo.storage`): each branch and tag of the git
-- repository an akt is, and the object it names. Its objects are packfiles
-- in object storage, under `git/<tenant>/<akt>/`. A ref only moves by a
-- compare-and-swap, in the transaction that writes what its push changed.
create table git_ref (
    tenant_id uuid not null,
    akt_id uuid not null references akt (id),
    name text not null,
    target text not null,
    primary key (tenant_id, akt_id, name)
);
