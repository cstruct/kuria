-- People a tenant names before, or without, them being users. A contact is
-- linked to the subject who first signs in to the tenant with its email,
-- verified. Its name and email are deleted when it is erased, or when the
-- subject it is linked to is; the row stays, so values naming it still
-- read.
create table contact (
    id uuid primary key,
    tenant_id uuid not null,
    name text check (name is null or length(btrim(name)) > 0),
    email text,
    linked_subject uuid,
    linked_at timestamptz,
    created_by uuid not null,
    created_at timestamptz not null,
    check ((linked_subject is null) = (linked_at is null))
);

create index contact_tenant on contact (tenant_id);
-- One contact per email per tenant, whatever its case.
create unique index contact_email on contact (tenant_id, lower(email))
    where email is not null;
create index contact_linked on contact (linked_subject)
    where linked_subject is not null;
