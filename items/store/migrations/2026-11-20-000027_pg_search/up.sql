-- pg_search (ParadeDB), for text search (`akt.search.text`), with pgvector,
-- which it requires. Creating it needs a superuser: where Kuria's role
-- isn't one, an operator creates it first, and this finds it there. The
-- server must also preload it (`shared_preload_libraries = 'pg_search'`).
create extension if not exists pg_search cascade;
