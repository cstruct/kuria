-- Uploaded files. The bytes are in object storage under
-- `<tenant>/<sha256 in hex>`, so the same file is stored once per tenant.
create table attachment (
    id uuid primary key,
    tenant_id uuid not null,
    name text not null,
    media_type text not null,
    size bigint not null check (size >= 0),
    sha256 bytea not null check (length(sha256) = 32),
    uploaded_by uuid not null,
    uploaded_at timestamptz not null,
    -- For an SBOM: its format, specification version, name and component
    -- count.
    sbom jsonb
);

create index attachment_tenant on attachment (tenant_id);

-- Which field of which akt holds which attachment, kept by the write path,
-- so that reading an attachment is decided in one query.
create table akt_attachment (
    tenant_id uuid not null,
    akt_id uuid not null references akt (id),
    field text not null,
    attachment_id uuid not null references attachment (id),
    primary key (akt_id, field)
);

create index akt_attachment_attachment on akt_attachment (attachment_id);
