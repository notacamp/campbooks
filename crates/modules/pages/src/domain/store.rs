//! Reading a page's body from its snapshot and the updates after it.

use campbooks_kernel::{Error, Result};
use sqlx::PgConnection;
use yrs::Doc;

use super::{PageId, document};

/// A page's document as stored.
pub struct Loaded {
    /// The document: the snapshot with every update after it applied.
    pub doc: Doc,
    /// The sequence number of the last update (or the snapshot's, when no
    /// update follows it); 0 for a page with no body yet.
    pub last_seq: i64,
    /// How many updates follow the snapshot.
    pub tail: i64,
}

/// The snapshot and the tail in one statement, so a compaction that commits
/// in between can never leave a reader with a snapshot and a tail that do
/// not meet.
const BODY: &str = "SELECT 's' AS kind, upto_seq AS seq, state AS data
       FROM pages.snapshots WHERE page_id = $1
     UNION ALL
     SELECT 'u', u.seq, u.update
       FROM pages.updates u
      WHERE u.page_id = $1
        AND u.seq > coalesce((SELECT upto_seq FROM pages.snapshots WHERE page_id = $1), 0)
     ORDER BY 1, 2";

/// Page `id`'s document.
pub async fn load(conn: &mut PgConnection, id: PageId) -> Result<Loaded> {
    let rows: Vec<(String, i64, Vec<u8>)> = sqlx::query_as(BODY).bind(id).fetch_all(conn).await?;
    let mut snapshot: Option<Vec<u8>> = None;
    let mut tail: Vec<Vec<u8>> = Vec::new();
    let mut last_seq = 0;
    for (kind, seq, data) in rows {
        last_seq = last_seq.max(seq);
        if kind == "s" {
            snapshot = Some(data);
        } else {
            tail.push(data);
        }
    }
    let doc = document::open(snapshot.as_deref(), &tail)
        .map_err(|_| Error::internal("a stored page body does not open"))?;
    Ok(Loaded {
        doc,
        last_seq,
        tail: i64::try_from(tail.len()).unwrap_or(i64::MAX),
    })
}
