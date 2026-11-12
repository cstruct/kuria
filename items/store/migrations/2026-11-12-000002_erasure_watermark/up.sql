-- The time Kuria has handled Konklav's erasures up to: the latest
-- `erased_at` read from Konklav's list of erased subjects. One row.
create table erasure_watermark (
    id smallint primary key default 1 check (id = 1),
    -- Unix seconds, as Konklav reports erasure times.
    handled_until bigint not null,
    updated_at timestamptz not null default now()
);
