-- Each akt's title, as its akt type version's presentation gives it, kept
-- by the write path beside the fields it comes from, so akter can be
-- searched and listed by title across akt types.
alter table akt add column title text;

-- The title field of each version: the presentation's, else the first
-- required single-line text field.
update akt
set title = nullif(btrim(akt.fields ->> titles.field), '')
from (
    select v.id,
        coalesce(
            v.definition -> 'presentation' ->> 'title',
            (
                select f ->> 'key'
                from jsonb_array_elements(v.definition -> 'fields') with ordinality as t(f, n)
                where f ->> 'kind' = 'text'
                  and coalesce((f ->> 'required')::boolean, false)
                  and not coalesce((f ->> 'multiline')::boolean, false)
                order by n
                limit 1
            )
        ) as field
    from akt_type_version v
) as titles
where titles.id = akt.type_version_id
  and titles.field is not null;

-- Searching by a number's beginning, whatever its case.
create index akt_number_lower on akt (tenant_id, lower(number) text_pattern_ops);
