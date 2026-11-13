-- Bundles (`bundle.*`): the aliases an import records for the subjects it
-- maps, and a record of each import.

-- A subject a bundle names, shown as and filtering as a user of the
-- tenant (`bundle.import.subjects`). Revisions keep naming `subject_id`.
create table subject_alias (
    tenant_id uuid not null,
    subject_id uuid not null,
    alias_of uuid not null,
    created_at timestamptz not null,
    primary key (tenant_id, subject_id),
    check (subject_id <> alias_of)
);

create index subject_alias_of on subject_alias (tenant_id, alias_of);

-- Who imported what, and when (`bundle.import.recorded`).
create table bundle_import (
    id uuid primary key,
    tenant_id uuid not null,
    imported_by uuid not null,
    imported_at timestamptz not null,
    scope text not null check (scope in ('types', 'tenant')),
    akt_types integer not null,
    akter integer not null
);

create index bundle_import_tenant on bundle_import (tenant_id, imported_at);
