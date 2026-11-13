-- Listing akter most recently changed first (`akt.list`).
create index akt_recent on akt (tenant_id, updated_at desc, id);
