# Kuria's Postgres: Postgres 18 with pg_search (ParadeDB), which text search
# needs (`akt.search.text`), and pgvector, which pg_search requires. Every
# Kuria database runs on it: development, tests, CI, the examples, dogfooding
# and the end-to-end tests.
FROM postgres:18.6-trixie

ARG TARGETARCH
ARG PG_SEARCH_VERSION=0.27.0

# pgvector from the PostgreSQL apt repository the image already uses;
# pg_search from ParadeDB's release, one .deb per Debian release and
# architecture.
RUN apt-get update \
    && apt-get install -y --no-install-recommends postgresql-18-pgvector \
    && rm -rf /var/lib/apt/lists/*
ADD https://github.com/paradedb/paradedb/releases/download/v${PG_SEARCH_VERSION}/postgresql-18-pg-search_${PG_SEARCH_VERSION}-1PARADEDB-trixie_${TARGETARCH}.deb /tmp/pg_search.deb
RUN apt-get update \
    && apt-get install -y --no-install-recommends /tmp/pg_search.deb \
    && rm -rf /var/lib/apt/lists/* /tmp/pg_search.deb

# pg_search must be preloaded.
CMD ["postgres", "-c", "shared_preload_libraries=pg_search"]
