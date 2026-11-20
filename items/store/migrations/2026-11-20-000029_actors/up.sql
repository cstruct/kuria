-- Who really made a change, when it was made as another subject: the
-- actor a token's actor chain (`act`) names, such as an impersonator or an
-- agent a person let act for them (`auth.actor`, `akt.revision.actor`).
-- Null when the subject made it themself.
alter table akt_revision add column actor uuid;
alter table akt_draft_update add column actor uuid;
