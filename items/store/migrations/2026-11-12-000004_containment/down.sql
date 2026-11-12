alter table change_event drop constraint change_event_kind;
delete from change_event where kind = 'rank';
alter table change_event alter column revision set not null;
alter table change_event drop column kind;
alter table akt_revision drop column membership;
drop table akt_membership;
drop index akt_rank;
alter table akt drop column rank;
