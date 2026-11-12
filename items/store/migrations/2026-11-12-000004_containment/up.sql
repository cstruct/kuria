-- Ranks: each akt's place in its tenant's one priority order. Keys are
-- fractional-indexing strings of 0-9A-Za-z, ordered byte by byte, hence the
-- "C" collation.
alter table akt add column rank text collate "C";

-- The key of the nth akt appended to an empty order, counting from 0, as
-- kuria_engine::nth_appended computes it: a0 … az, b00 … bzz, c000 …
create function kuria_nth_appended(n bigint) returns text
language plpgsql immutable as $$
declare
    digits constant text := '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
    width int := 1;
    rest bigint := n;
    key text := '';
begin
    while rest >= power(62, width)::bigint loop
        rest := rest - power(62, width)::bigint;
        width := width + 1;
    end loop;
    for i in 1..width loop
        key := substr(digits, (rest % 62)::int + 1, 1) || key;
        rest := rest / 62;
    end loop;
    return chr(ascii('a') + width - 1) || key;
end;
$$;

update akt set rank = ranked.key
from (
    select id, kuria_nth_appended(row_number() over (partition by tenant_id order by created_at, id) - 1) as key
    from akt
) as ranked
where akt.id = ranked.id;

drop function kuria_nth_appended;

alter table akt alter column rank set not null;
create unique index akt_rank on akt (tenant_id, rank);

-- Current memberships. Their history is in the akt revisions of both akter.
create table akt_membership (
    tenant_id uuid not null,
    container_id uuid not null references akt (id),
    member_id uuid not null references akt (id),
    added_at timestamptz not null,
    added_by uuid not null,
    primary key (container_id, member_id),
    check (container_id <> member_id)
);

create index akt_membership_member on akt_membership (member_id);

-- A membership revision records the other akt:
-- {"added": {"akt": "<id>", "number": "…"}} on the member, and
-- {"added_member": {…}} on the container; `removed` and `removed_member`
-- likewise.
alter table akt_revision add column membership jsonb;

-- Change events are now of two kinds: a new revision, or a new rank, which
-- has no revision.
alter table change_event add column kind text not null default 'revision';
alter table change_event alter column kind drop default;
alter table change_event alter column revision drop not null;
alter table change_event add constraint change_event_kind check (
    (kind = 'revision' and revision is not null) or (kind = 'rank' and revision is null)
);
