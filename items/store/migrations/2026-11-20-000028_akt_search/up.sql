-- Text search (`akt.search.text`): a row per text and `markdown` field of
-- each akt as it reads, its live draft's for an akt with one, under a BM25
-- index of pg_search. Triggers on `akt` and `akt_draft` keep it in the
-- transaction of each change, whatever wrote it, so a change is searchable
-- once committed (`akt.search.text.match`).
create table akt_search (
    id bigint generated always as identity primary key,
    tenant_id uuid not null,
    akt_id uuid not null references akt_number (id) on delete cascade,
    -- The akt type's key, so a search reads only the types the subject may.
    type_key text not null,
    field text not null,
    archived boolean not null,
    text text not null,
    unique (akt_id, field)
);

-- Words split on anything not a letter or digit, lowercased and stemmed
-- as English; the filters are terms of the index, so they narrow the scan.
create index akt_search_text on akt_search using paradedb (
    id,
    (text::pdb.simple('stemmer=english')),
    tenant_id,
    (type_key::pdb.literal),
    archived,
    akt_id
);

-- Makes `akt_search` hold what `target` reads as now: each text field of
-- its akt type, from its live draft if it has one stored, else its row.
create function akt_search_refresh(target uuid) returns void
language plpgsql as $$
declare
    found_akt record;
    source jsonb;
begin
    select akt.tenant_id, akt.archived, akt.fields, version.key, version.definition
    into found_akt
    from akt
    join akt_type_version version on version.id = akt.type_version_id
    where akt.id = target;
    if found_akt is null then
        delete from akt_search where akt_id = target;
        return;
    end if;
    select draft.fields into source from akt_draft draft where draft.akt_id = target;
    source := coalesce(source, found_akt.fields);
    delete from akt_search
    where akt_id = target
      and field not in (
          select f ->> 'key'
          from jsonb_array_elements(found_akt.definition -> 'fields') f
          where f ->> 'kind' in ('text', 'markdown')
            and jsonb_typeof(source -> (f ->> 'key')) = 'string'
            and source ->> (f ->> 'key') <> ''
      );
    insert into akt_search (tenant_id, akt_id, type_key, field, archived, text)
    select found_akt.tenant_id, target, found_akt.key, f ->> 'key', found_akt.archived,
           source ->> (f ->> 'key')
    from jsonb_array_elements(found_akt.definition -> 'fields') f
    where f ->> 'kind' in ('text', 'markdown')
      and jsonb_typeof(source -> (f ->> 'key')) = 'string'
      and source ->> (f ->> 'key') <> ''
    on conflict (akt_id, field) do update
        set text = excluded.text, archived = excluded.archived, type_key = excluded.type_key
        where akt_search.text is distinct from excluded.text
           or akt_search.archived is distinct from excluded.archived
           or akt_search.type_key is distinct from excluded.type_key;
end;
$$;

create function akt_search_from_akt() returns trigger
language plpgsql as $$
begin
    perform akt_search_refresh(coalesce(new.id, old.id));
    return null;
end;
$$;

create function akt_search_from_draft() returns trigger
language plpgsql as $$
begin
    perform akt_search_refresh(coalesce(new.akt_id, old.akt_id));
    return null;
end;
$$;

-- Archiving moves an akt between partitions as a delete and an insert;
-- each refreshes, the last as it ends up.
create trigger akt_search_akt
    after insert or update of fields, archived, type_version_id or delete on akt
    for each row execute function akt_search_from_akt();

create trigger akt_search_draft
    after insert or update of fields or delete on akt_draft
    for each row execute function akt_search_from_draft();

select akt_search_refresh(id) from akt;
