-- Live drafts (`live.*`): the Yjs document of each akt whose type has one.
-- `state` is the draft as of its update `compacted_seq`, encoded as one Yjs
-- v1 update; the updates after it make the draft as it is. `fields` is
-- what the whole draft reads as, kept for reads (`live.read`).
-- `released_seq` is the last update the latest release took.
create table akt_draft (
    tenant_id uuid not null,
    akt_id uuid primary key references akt (id),
    state bytea not null,
    fields jsonb not null,
    compacted_seq bigint not null default 0,
    released_seq bigint not null default 0,
    updated_at timestamptz not null
);

-- Every change to a live draft, as the Yjs v1 update that made it, with who
-- made it, when, and the keys of the fields it changed (`live.history`).
-- Never changed or removed: compacting moves `akt_draft.state` on and keeps
-- these.
create table akt_draft_update (
    tenant_id uuid not null,
    akt_id uuid not null references akt (id),
    seq bigint not null check (seq > 0),
    update bytea not null,
    by uuid not null,
    at timestamptz not null,
    fields jsonb not null default '[]',
    primary key (akt_id, seq)
);

create function akt_draft_update_is_immutable() returns trigger
language plpgsql as $$
begin
    raise exception 'live draft updates never change';
end;
$$;

create trigger akt_draft_update_immutable
    before update or delete on akt_draft_update
    for each row execute function akt_draft_update_is_immutable();
