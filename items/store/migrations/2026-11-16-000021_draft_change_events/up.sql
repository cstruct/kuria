-- A change to an akt's live draft is a change event of its own
-- (`live.sync`), naming the akt's latest revision, which it leaves as it is.
alter table change_event drop constraint change_event_kind;
alter table change_event add constraint change_event_kind check (
    (kind in ('revision', 'version', 'draft') and revision is not null)
    or (kind = 'rank' and revision is null)
);
