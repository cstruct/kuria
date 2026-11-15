-- What each copy was copied from (`akt.copy.record`): one row per akt made
-- by copying another, kept apart from the akt itself, which a copy is in
-- every other way.
create table akt_origin (
    tenant_id uuid not null,
    akt_id uuid primary key references akt (id),
    origin_id uuid not null references akt (id),
    check (akt_id <> origin_id)
);

create index akt_origin_of on akt_origin (origin_id);
