-- Back to unpartitioned tables, archived akter kept as live ones.
do $$
declare
    fk record;
begin
    for fk in
        select conrelid::regclass as tab, conname
        from pg_constraint
        where contype = 'f'
          and confrelid in ('akt'::regclass, 'akt_revision'::regclass, 'akt_number'::regclass)
          and conrelid not in ('akt'::regclass)
    loop
        execute format('alter table %s drop constraint %I', fk.tab, fk.conname);
    end loop;
end;
$$;

alter table akt_version drop column akt_archived;
alter table akt_commit drop column akt_archived;

create table akt_plain as
select id, tenant_id, number, type_version_id, state, fields, revision,
    created_at, updated_at, rank, title
from akt;
create table akt_revision_plain as
select akt_id, revision, tenant_id, author, transition, reason, changes, state,
    hash, created_at, membership
from akt_revision;
create table akt_draft_update_plain as
select tenant_id, akt_id, seq, update, by, at, fields, message from akt_draft_update;
create table change_event_plain as
select tenant_id, seq, akt_id, revision, created_at, published_at, kind from change_event;

drop table change_event;
drop table akt_draft_update;
drop table akt_revision;
drop table akt;
drop table akt_number;
drop function akt_number_is_kept;
drop function kuria_archiving;

alter table akt_plain rename to akt;
alter table akt add primary key (id);
alter table akt add unique (tenant_id, number);
alter table akt add foreign key (type_version_id) references akt_type_version (id);
alter table akt alter column rank set not null;
create index akt_tenant_type on akt (tenant_id, type_version_id);
create index akt_fields on akt using gin (fields);
create index akt_number_lower on akt (tenant_id, lower(number) text_pattern_ops);
create index akt_recent on akt (tenant_id, updated_at desc, id);
create unique index akt_rank on akt (tenant_id, rank);

alter table akt_revision_plain rename to akt_revision;
alter table akt_revision add primary key (akt_id, revision);
alter table akt_revision add foreign key (akt_id) references akt (id);

create or replace function akt_revision_is_append_only() returns trigger
language plpgsql as $$
begin
    raise exception 'akt revisions are append only';
end;
$$;
create trigger akt_revision_append_only
    before update or delete on akt_revision
    for each row execute function akt_revision_is_append_only();

alter table akt_draft_update_plain rename to akt_draft_update;
alter table akt_draft_update add primary key (akt_id, seq);
alter table akt_draft_update add foreign key (akt_id) references akt (id);
alter table akt_draft_update alter column fields set default '[]';

create or replace function akt_draft_update_is_immutable() returns trigger
language plpgsql as $$
begin
    raise exception 'live draft updates never change';
end;
$$;
create trigger akt_draft_update_immutable
    before update or delete on akt_draft_update
    for each row execute function akt_draft_update_is_immutable();

alter table change_event_plain rename to change_event;
alter table change_event add primary key (tenant_id, seq);
alter table change_event add foreign key (akt_id, revision) references akt_revision (akt_id, revision);
alter table change_event add constraint change_event_kind check (
    (kind in ('revision', 'version', 'draft') and revision is not null)
    or (kind = 'rank' and revision is null)
);
create index change_event_unpublished on change_event (created_at) where published_at is null;

create or replace function akt_version_is_immutable() returns trigger
language plpgsql as $$
begin
    raise exception 'akt versions never change';
end;
$$;

alter table akt_version add foreign key (akt_id, revision) references akt_revision (akt_id, revision);
alter table akt_commit add foreign key (akt_id, revision) references akt_revision (akt_id, revision);
alter table akt_membership add foreign key (container_id) references akt (id);
alter table akt_membership add foreign key (member_id) references akt (id);
alter table akt_attachment add foreign key (akt_id) references akt (id);
alter table akt_link add foreign key (source_id) references akt (id);
alter table akt_link add foreign key (target_id) references akt (id);
alter table akt_origin add foreign key (akt_id) references akt (id);
alter table akt_origin add foreign key (origin_id) references akt (id);
alter table akt_version add foreign key (akt_id) references akt (id);
alter table akt_held add foreign key (akt_id) references akt (id);
alter table akt_diagram add foreign key (akt_id) references akt (id);
alter table akt_draft add foreign key (akt_id) references akt (id);
