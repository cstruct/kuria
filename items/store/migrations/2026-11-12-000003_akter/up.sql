-- Per-tenant counters: akt number sequences (`number:<scope>`) and the
-- change-event sequence (`change_event`). Taking a number locks the row
-- until the transaction ends, so numbers follow commit order.
create table tenant_counter (
    tenant_id uuid not null,
    name text not null,
    value bigint not null,
    primary key (tenant_id, name)
);

-- One row per akt type key, naming its latest version.
create table akt_type (
    tenant_id uuid not null,
    key text not null,
    latest_version integer not null,
    primary key (tenant_id, key)
);

-- Every version of every akt type, as submitted. Never updated.
create table akt_type_version (
    id uuid primary key,
    tenant_id uuid not null,
    key text not null,
    version integer not null,
    definition jsonb not null,
    created_by uuid not null,
    created_at timestamptz not null default now(),
    unique (tenant_id, key, version),
    foreign key (tenant_id, key) references akt_type (tenant_id, key)
);

-- The current state of each akt: a projection of its revisions, written in
-- the same transaction as each of them.
create table akt (
    id uuid primary key,
    tenant_id uuid not null,
    number text not null,
    type_version_id uuid not null references akt_type_version (id),
    state text not null,
    fields jsonb not null,
    revision integer not null,
    created_at timestamptz not null,
    updated_at timestamptz not null,
    unique (tenant_id, number)
);

create index akt_tenant_type on akt (tenant_id, type_version_id);
create index akt_fields on akt using gin (fields);

-- Every change to every akt. Append only.
create table akt_revision (
    akt_id uuid not null references akt (id),
    revision integer not null check (revision >= 1),
    tenant_id uuid not null,
    author uuid not null,
    transition text,
    reason text,
    -- The field values the change set; `null` for a cleared field.
    changes jsonb not null,
    state text not null,
    -- SHA-256 of the resulting state and field values, as canonical JSON.
    hash bytea not null check (length(hash) = 32),
    created_at timestamptz not null,
    primary key (akt_id, revision)
);

-- Revisions are records: no statement may change or remove one.
create function akt_revision_is_append_only() returns trigger
language plpgsql as $$
begin
    raise exception 'akt revisions are append only';
end;
$$;

create trigger akt_revision_append_only
    before update or delete on akt_revision
    for each row execute function akt_revision_is_append_only();

-- The outbox of change events, numbered per tenant in commit order.
create table change_event (
    tenant_id uuid not null,
    seq bigint not null,
    akt_id uuid not null,
    revision integer not null,
    created_at timestamptz not null,
    published_at timestamptz,
    primary key (tenant_id, seq),
    foreign key (akt_id, revision) references akt_revision (akt_id, revision)
);

create index change_event_unpublished on change_event (created_at) where published_at is null;
