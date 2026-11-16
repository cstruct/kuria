-- Each diagram block an akt's `markdown` field rendered
-- (`akt.field.markdown.diagrams`): the SVG, an attachment the akt holds
-- (`akt_held`), by the field and the hash of the block's source as
-- rendered. Never removed, so every revision and version shows its own.
create table akt_diagram (
    tenant_id uuid not null,
    akt_id uuid not null references akt (id),
    field text not null,
    source_sha256 bytea not null,
    attachment_id uuid not null references attachment (id),
    primary key (akt_id, field, source_sha256)
);

create trigger akt_diagram_immutable
    before update or delete on akt_diagram
    for each row execute function akt_version_is_immutable();
