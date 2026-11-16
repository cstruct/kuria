create table git_ref (
    tenant_id uuid not null,
    akt_id uuid not null references akt (id),
    name text not null,
    target text not null,
    primary key (tenant_id, akt_id, name)
);

create table git_mark (
    tenant_id uuid not null,
    repository_id uuid not null references akt (id),
    akt_id uuid not null references akt (id),
    revision integer not null,
    draft_seq bigint not null,
    primary key (tenant_id, repository_id, akt_id)
);
