-- Versions (`version.*`): revisions of an akt named by releasing them,
-- numbered per akt, never changed or removed.
create table akt_version (
    tenant_id uuid not null,
    akt_id uuid not null references akt (id),
    number integer not null check (number > 0),
    revision integer not null check (revision > 0),
    label text check (label is null or (length(label) <= 100 and label !~ '[\r\n]')),
    released_by uuid not null,
    released_at timestamptz not null,
    -- What each reference naming no version meant at release
    -- (`version.pins`): `[{"destination", "number", "version"?, "revision"?}]`.
    pins jsonb not null default '[]',
    primary key (akt_id, number),
    unique (akt_id, revision),
    foreign key (akt_id, revision) references akt_revision (akt_id, revision)
);

create function akt_version_is_immutable() returns trigger
language plpgsql as $$
begin
    raise exception 'akt versions never change';
end;
$$;

create trigger akt_version_immutable
    before update or delete on akt_version
    for each row execute function akt_version_is_immutable();

-- A release is a change event of its own (`version.history`), naming the
-- revision the version names.
alter table change_event drop constraint change_event_kind;
alter table change_event add constraint change_event_kind check (
    (kind in ('revision', 'version') and revision is not null)
    or (kind = 'rank' and revision is null)
);
