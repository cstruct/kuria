-- A version's optional description (`version.release`): what changed, in
-- text of up to 2,000 characters. Set as the version is inserted, which
-- never changes after.
alter table akt_version
    add column description text
        check (description is null or length(description) <= 2000);
