alter table change_event drop constraint change_event_kind;
delete from change_event where kind = 'draft';
alter table change_event add constraint change_event_kind check (
    (kind in ('revision', 'version') and revision is not null)
    or (kind = 'rank' and revision is null)
);
