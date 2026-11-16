-- The git commit a pushed batch came from (`git.identity`): its author and
-- committer lines and message, kept with each revision it made, so that
-- every clone builds that very commit. Revisions are append only, so it is
-- kept beside them.
create table akt_commit (
    tenant_id uuid not null,
    akt_id uuid not null,
    revision integer not null,
    commit jsonb not null,
    primary key (akt_id, revision),
    foreign key (akt_id, revision) references akt_revision (akt_id, revision)
);

create trigger akt_commit_immutable
    before update or delete on akt_commit
    for each row execute function akt_version_is_immutable();
