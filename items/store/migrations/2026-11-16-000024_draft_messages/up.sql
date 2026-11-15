-- A push from a clone keeps its message with the one change it makes to a
-- live draft (`clone.server`, `live.history`); live typing has none.
alter table akt_draft_update add column message text;
