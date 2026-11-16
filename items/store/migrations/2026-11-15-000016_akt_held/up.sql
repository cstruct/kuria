-- Every attachment an akt has held, in a field at any revision or as a
-- diagram (`attachment.download`): never let go of, so that a revision or
-- version read later can still show it. `akt_attachment` says what its
-- fields hold now.
create table akt_held (
    tenant_id uuid not null,
    akt_id uuid not null references akt (id),
    attachment_id uuid not null references attachment (id),
    primary key (akt_id, attachment_id)
);

create index akt_held_attachment on akt_held (attachment_id);

insert into akt_held (tenant_id, akt_id, attachment_id)
select tenant_id, akt_id, attachment_id from akt_attachment
on conflict do nothing;
