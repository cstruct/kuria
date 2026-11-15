-- Hosted repositories are gone: clones sync with Kuria through `kuria`
-- (`clone.*`), so neither refs nor what `main` held of Kuria are kept.
drop table git_mark;
drop table git_ref;
