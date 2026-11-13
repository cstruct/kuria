-- The tenants each subject signed in to through a browser (`person.tenant`),
-- so that a tenant's users can be listed and a contact linked to one by
-- hand. Backfilled from the sessions there are.
create table tenant_person (
    tenant_id uuid not null,
    subject_id uuid not null,
    first_seen_at timestamptz not null,
    last_seen_at timestamptz not null,
    primary key (tenant_id, subject_id)
);

create index tenant_person_subject on tenant_person (subject_id);

insert into tenant_person (tenant_id, subject_id, first_seen_at, last_seen_at)
select tenant_id, subject_id, min(created_at), max(coalesce(refreshed_at, created_at))
from browser_session
group by tenant_id, subject_id;

-- Who linked a contact by hand (`contact.link-by-hand`); null when it was
-- linked by a verified email at sign-in.
alter table contact add column linked_by uuid;
