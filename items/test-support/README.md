# kuria-test-support

Test helpers shared by Kuria's software items.

- `TestDatabase` creates a uniquely named, migrated database on the
  development Postgres server and drops it again when it goes out of scope.
  The server comes from `KURIA_TEST_DATABASE_URL`, or `DATABASE_URL` as mise
  sets it. Database tests run on a multi-threaded runtime
  (`#[tokio::test(flavor = "multi_thread")]`), because the migration harness
  blocks its thread. `TestDatabase::unmigrated()` applies none, for tests
  that step through migrations with `kuria_store::run_next_migration`.
- `valkey_url()` gives the development Valkey, from `KURIA_TEST_VALKEY_URL`
  or `KURIA_VALKEY_URL`, and `unique_name(prefix)` a name no other test
  uses, so tests sharing Valkey keep their rate limits apart.
