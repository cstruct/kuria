-- What a repository's `main` holds of Kuria (`git.repo.commits`): for each
-- akt of it, the revision and the live draft's update its file on `main` is
-- as of. Kuria's changes after these are what its next commit makes, and
-- their authors are its authors.
create table git_mark (
    tenant_id uuid not null,
    repository_id uuid not null references akt (id),
    akt_id uuid not null references akt (id),
    revision integer not null,
    draft_seq bigint not null,
    primary key (tenant_id, repository_id, akt_id)
);
