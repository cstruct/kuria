-- Typed links between akter besides containment (`link.*`): one row per
-- link of one kind from one akt to another, with any target revision it
-- pins. Adding and removing one are akt revisions of both akter.
create table akt_link (
    tenant_id uuid not null,
    source_id uuid not null references akt (id),
    kind text not null,
    target_id uuid not null references akt (id),
    pinned_revision integer,
    added_at timestamptz not null,
    added_by uuid not null,
    primary key (source_id, kind, target_id),
    check (source_id <> target_id)
);

create index akt_link_target on akt_link (target_id);
