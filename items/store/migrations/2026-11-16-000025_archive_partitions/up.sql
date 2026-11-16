-- Archiving (`archive.*`): every akt live or archived, kept in partitions
-- of its own (`archive.storage`), with its revisions, live-draft updates
-- and change events beside it.
--
-- * `akt_number` holds each akt's id and number, unpartitioned: numbers
--   stay unique across partitions, and the tables that only name an akt
--   reference it there.
-- * `akt`, `akt_revision`, `akt_draft_update` and `change_event` are
--   list-partitioned on whether the akt is archived; the last three
--   reference `akt (id, archived)` `on update cascade`, and versions and
--   commits their revision, so archiving an akt moves all of it.
-- * Postgres moves a row between partitions as a delete and an insert, so
--   the append-only triggers let a delete through while `kuria.archiving`
--   is set for the transaction, and an update that changes only the
--   archived column.

-- Every reference to akt and akt_revision goes, to come back below.
do $$
declare
    fk record;
begin
    for fk in
        select conrelid::regclass as tab, conname
        from pg_constraint
        where contype = 'f'
          and confrelid in ('akt'::regclass, 'akt_revision'::regclass)
    loop
        execute format('alter table %s drop constraint %I', fk.tab, fk.conname);
    end loop;
end;
$$;

-- Numbers.
create table akt_number (
    id uuid primary key,
    tenant_id uuid not null,
    number text not null,
    unique (tenant_id, number)
);

insert into akt_number (id, tenant_id, number) select id, tenant_id, number from akt;

-- Akter.
alter table akt rename to akt_unpartitioned;

create table akt (
    id uuid not null references akt_number (id),
    tenant_id uuid not null,
    number text not null,
    type_version_id uuid not null references akt_type_version (id),
    state text not null,
    fields jsonb not null,
    revision integer not null,
    created_at timestamptz not null,
    updated_at timestamptz not null,
    rank text collate "C" not null,
    title text,
    -- When it reached its state (`archive.rules.state-since`).
    state_since timestamptz not null,
    -- `archive.flag`: when, by whom (`{"subject": id}` or `{"rule": kind}`)
    -- and why; `archive_kept` marks one a person unarchived after a rule
    -- archived it (`archive.rules.kept`).
    archived boolean not null default false,
    archived_at timestamptz,
    archived_by jsonb,
    archive_reason text,
    archive_kept boolean not null default false,
    primary key (id, archived),
    check (archived = (archived_at is not null)),
    check (archived = (archived_by is not null))
) partition by list (archived);

create table akt_live partition of akt for values in (false);
create table akt_archived partition of akt for values in (true);

insert into akt (
    id, tenant_id, number, type_version_id, state, fields, revision,
    created_at, updated_at, rank, title, state_since
)
select a.id, a.tenant_id, a.number, a.type_version_id, a.state, a.fields, a.revision,
    a.created_at, a.updated_at, a.rank, a.title,
    coalesce(
        (select max(r.created_at) from akt_revision r
         where r.akt_id = a.id and r.transition is not null),
        a.created_at
    )
from akt_unpartitioned a;

drop table akt_unpartitioned;

create index akt_tenant_type on akt (tenant_id, type_version_id);
create index akt_fields on akt using gin (fields);
create index akt_number_lower on akt (tenant_id, lower(number) text_pattern_ops);
create index akt_recent on akt (tenant_id, updated_at desc, id);
-- Ranks order live akter; an archived akt's rank is checked again when it
-- comes back.
create unique index akt_live_rank on akt_live (tenant_id, rank);
create index akt_archived_rank on akt_archived (tenant_id, rank);

-- Each akt's number, kept with it. A move between partitions inserts the
-- row again, its number already there.
create function akt_number_is_kept() returns trigger
language plpgsql as $$
begin
    insert into akt_number (id, tenant_id, number) values (new.id, new.tenant_id, new.number)
    on conflict (id) do nothing;
    return new;
end;
$$;

create trigger akt_number_kept
    before insert on akt
    for each row execute function akt_number_is_kept();

-- Whether a row may change: only its archived column, and only a delete
-- that is half of a move, while archiving.
create function kuria_archiving() returns boolean
language sql stable as $$
    select coalesce(current_setting('kuria.archiving', true), '') = 'on'
$$;

-- Revisions.
alter table akt_revision rename to akt_revision_unpartitioned;

create table akt_revision (
    akt_id uuid not null,
    revision integer not null check (revision >= 1),
    tenant_id uuid not null,
    author uuid not null,
    transition text,
    reason text,
    changes jsonb not null,
    state text not null,
    hash bytea not null check (length(hash) = 32),
    created_at timestamptz not null,
    membership jsonb,
    akt_archived boolean not null default false,
    primary key (akt_id, revision, akt_archived),
    foreign key (akt_id, akt_archived) references akt (id, archived) on update cascade
) partition by list (akt_archived);

create table akt_revision_live partition of akt_revision for values in (false);
create table akt_revision_archived partition of akt_revision for values in (true);

insert into akt_revision (
    akt_id, revision, tenant_id, author, transition, reason, changes, state,
    hash, created_at, membership
)
select akt_id, revision, tenant_id, author, transition, reason, changes, state,
    hash, created_at, membership
from akt_revision_unpartitioned;

drop table akt_revision_unpartitioned;

create or replace function akt_revision_is_append_only() returns trigger
language plpgsql as $$
begin
    if tg_op = 'UPDATE' and to_jsonb(new) - 'akt_archived' = to_jsonb(old) - 'akt_archived' then
        return new;
    end if;
    if tg_op = 'DELETE' and kuria_archiving() then
        return old;
    end if;
    raise exception 'akt revisions are append only';
end;
$$;

create trigger akt_revision_append_only
    before update or delete on akt_revision
    for each row execute function akt_revision_is_append_only();

-- Live-draft updates.
alter table akt_draft_update rename to akt_draft_update_unpartitioned;

create table akt_draft_update (
    tenant_id uuid not null,
    akt_id uuid not null,
    seq bigint not null check (seq > 0),
    update bytea not null,
    by uuid not null,
    at timestamptz not null,
    fields jsonb not null default '[]',
    message text,
    akt_archived boolean not null default false,
    primary key (akt_id, seq, akt_archived),
    foreign key (akt_id, akt_archived) references akt (id, archived) on update cascade
) partition by list (akt_archived);

create table akt_draft_update_live partition of akt_draft_update for values in (false);
create table akt_draft_update_archived partition of akt_draft_update for values in (true);

insert into akt_draft_update (tenant_id, akt_id, seq, update, by, at, fields, message)
select tenant_id, akt_id, seq, update, by, at, fields, message from akt_draft_update_unpartitioned;

drop table akt_draft_update_unpartitioned;

create or replace function akt_draft_update_is_immutable() returns trigger
language plpgsql as $$
begin
    if tg_op = 'UPDATE' and to_jsonb(new) - 'akt_archived' = to_jsonb(old) - 'akt_archived' then
        return new;
    end if;
    if tg_op = 'DELETE' and kuria_archiving() then
        return old;
    end if;
    raise exception 'live draft updates never change';
end;
$$;

create trigger akt_draft_update_immutable
    before update or delete on akt_draft_update
    for each row execute function akt_draft_update_is_immutable();

-- Change events: the outbox the relay publishes in order, whichever
-- partition an event is in.
alter table change_event rename to change_event_unpartitioned;
alter index change_event_unpublished rename to change_event_unpublished_unpartitioned;

create table change_event (
    tenant_id uuid not null,
    seq bigint not null,
    akt_id uuid not null,
    revision integer,
    created_at timestamptz not null,
    published_at timestamptz,
    kind text not null,
    akt_archived boolean not null default false,
    primary key (tenant_id, seq, akt_archived),
    foreign key (akt_id, akt_archived) references akt (id, archived) on update cascade,
    constraint change_event_kind check (
        (kind in ('revision', 'version', 'draft') and revision is not null)
        or (kind = 'rank' and revision is null)
    )
) partition by list (akt_archived);

create table change_event_live partition of change_event for values in (false);
create table change_event_archived partition of change_event for values in (true);

insert into change_event (tenant_id, seq, akt_id, revision, created_at, published_at, kind)
select tenant_id, seq, akt_id, revision, created_at, published_at, kind
from change_event_unpartitioned;

drop table change_event_unpartitioned;

create index change_event_unpublished on change_event (created_at) where published_at is null;
create index change_event_seq on change_event (tenant_id, seq);

-- Versions and commits follow their revision.
alter table akt_version add column akt_archived boolean not null default false;
alter table akt_version add foreign key (akt_id, revision, akt_archived)
    references akt_revision (akt_id, revision, akt_archived) on update cascade;
alter table akt_commit add column akt_archived boolean not null default false;
alter table akt_commit add foreign key (akt_id, revision, akt_archived)
    references akt_revision (akt_id, revision, akt_archived) on update cascade;

create or replace function akt_version_is_immutable() returns trigger
language plpgsql as $$
begin
    if tg_op = 'UPDATE' and to_jsonb(new) - 'akt_archived' = to_jsonb(old) - 'akt_archived' then
        return new;
    end if;
    raise exception 'akt versions never change';
end;
$$;

-- Everything else names an akt by its number's row.
alter table akt_membership add foreign key (container_id) references akt_number (id);
alter table akt_membership add foreign key (member_id) references akt_number (id);
alter table akt_attachment add foreign key (akt_id) references akt_number (id);
alter table akt_link add foreign key (source_id) references akt_number (id);
alter table akt_link add foreign key (target_id) references akt_number (id);
alter table akt_origin add foreign key (akt_id) references akt_number (id);
alter table akt_origin add foreign key (origin_id) references akt_number (id);
alter table akt_version add foreign key (akt_id) references akt_number (id);
alter table akt_held add foreign key (akt_id) references akt_number (id);
alter table akt_diagram add foreign key (akt_id) references akt_number (id);
alter table akt_draft add foreign key (akt_id) references akt_number (id);
