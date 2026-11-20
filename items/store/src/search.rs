//! Text search (`akt.search.text`): `pg_search`'s BM25 over `akt_search`, a
//! row per text and `markdown` field of each akt as it reads, which
//! triggers keep (migration `000028_akt_search`).

use diesel::QueryableByName;
use diesel::sql_types::{Array, Bool, Double, Int8, Text, Uuid as SqlUuid};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::akter::Archived;

/// What marks each word found in an excerpt, before and after it.
pub const MARKS: (&str, &str) = ("«", "»");

/// One field of one akt that a text search matched.
#[derive(Clone, Debug, PartialEq, QueryableByName)]
pub struct TextMatch {
    #[diesel(sql_type = SqlUuid)]
    pub akt_id: Uuid,
    #[diesel(sql_type = Text)]
    pub field: String,
    /// Its BM25 score: higher is more relevant.
    #[diesel(sql_type = Double)]
    pub relevance: f64,
    /// A part of the field around the words found, each marked with
    /// [`MARKS`].
    #[diesel(sql_type = Text)]
    pub excerpt: String,
}

/// The fields of the tenant's akter of the akt types `keys`, live or
/// archived as `archived` says, that hold every word of `text` as the
/// index splits and stems them, most relevant first, at most `limit`.
// r[impl akt.search.text]
// r[impl akt.search.text.match]
pub async fn search_text(
    conn: &mut Connection,
    tenant: Uuid,
    text: &str,
    keys: &[String],
    archived: Archived,
    limit: i64,
) -> diesel::QueryResult<Vec<TextMatch>> {
    if text.trim().is_empty() || keys.is_empty() {
        return Ok(Vec::new());
    }
    diesel::sql_query(
        "select akt_id, field, pdb.score(id)::float8 as relevance,
                pdb.snippet(text, start_tag => $6, end_tag => $7, max_num_chars => 200) as excerpt
         from akt_search
         where text &&& $2
           and tenant_id = $1
           and type_key = any($3)
           and archived = any($4)
         order by relevance desc, id desc
         limit $5",
    )
    .bind::<SqlUuid, _>(tenant)
    .bind::<Text, _>(text)
    .bind::<Array<Text>, _>(keys)
    .bind::<Array<Bool>, _>(archived.values())
    .bind::<Int8, _>(limit)
    .bind::<Text, _>(MARKS.0)
    .bind::<Text, _>(MARKS.1)
    .load(conn)
    .await
}
