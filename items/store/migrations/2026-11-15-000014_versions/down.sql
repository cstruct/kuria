alter table change_event drop constraint change_event_kind;
delete from change_event where kind = 'version';
alter table change_event add constraint change_event_kind check (
    (kind = 'revision' and revision is not null) or (kind = 'rank' and revision is null)
);
drop trigger akt_version_immutable on akt_version;
drop function akt_version_is_immutable();
drop table akt_version;
