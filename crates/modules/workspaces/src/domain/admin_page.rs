//! Paging and search for the admin lists (ADR 0015): newest first, by an
//! opaque cursor (`?after=`) and a page size (`?limit=`, 1 to
//! [`MAX_LIMIT`], default [`DEFAULT_LIMIT`]), optionally narrowed to names or
//! addresses starting with `?q=`.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{Error, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::MESSAGES;

/// Rows a page holds unless `limit` says otherwise.
pub const DEFAULT_LIMIT: u16 = 50;
/// Most rows one page holds.
pub const MAX_LIMIT: u16 = 200;
/// Longest search text, in characters.
const MAX_QUERY_CHARS: usize = 100;

/// Which page of an admin list to return.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AdminPage {
    /// Only rows whose name or address starts with this (case-insensitive).
    pub q: Option<String>,
    /// The `next` cursor of the previous page.
    pub after: Option<String>,
    /// How many rows: 1 to 200, default 50.
    pub limit: Option<u16>,
}

/// An [`AdminPage`] checked and decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paging {
    /// A `LIKE` pattern for the prefix (wildcards escaped), if any.
    pub prefix: Option<String>,
    /// `(created_at, id)` of the last row of the previous page.
    pub after: Option<(DateTime<Utc>, Uuid)>,
    /// The page size.
    pub limit: u16,
}

impl Paging {
    /// Rows to fetch: one more than the page, to know whether another
    /// follows.
    pub fn fetch(&self) -> i64 {
        i64::from(self.limit) + 1
    }

    /// The cursor's time, for SQL.
    pub fn after_at(&self) -> Option<DateTime<Utc>> {
        self.after.map(|(at, _)| at)
    }

    /// The cursor's id, for SQL.
    pub fn after_id(&self) -> Option<Uuid> {
        self.after.map(|(_, id)| id)
    }

    /// Cut `rows` (fetched with [`fetch`](Self::fetch)) to the page and
    /// return the cursor of the next one, if any. `key` gives a row's
    /// `(created_at, id)`.
    pub fn finish<T>(
        &self,
        mut rows: Vec<T>,
        key: impl Fn(&T) -> (DateTime<Utc>, Uuid),
    ) -> (Vec<T>, Option<String>) {
        if rows.len() <= usize::from(self.limit) {
            return (rows, None);
        }
        rows.truncate(usize::from(self.limit));
        let next = rows.last().map(|row| {
            let (at, id) = key(row);
            encode_cursor(at, id)
        });
        (rows, next)
    }
}

impl AdminPage {
    /// Check the parameters: 400 `invalid_limit`, `invalid_cursor`, or
    /// `query_too_long`.
    pub fn paging(&self) -> Result<Paging> {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        if limit == 0 || limit > MAX_LIMIT {
            return Err(Error::BadRequest(
                MESSAGES.reason("invalid_limit").arg("max", MAX_LIMIT),
            ));
        }
        let after = self.after.as_deref().map(decode_cursor).transpose()?;
        let prefix = match self.q.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(q) if q.chars().count() > MAX_QUERY_CHARS => {
                return Err(Error::BadRequest(
                    MESSAGES
                        .reason("query_too_long")
                        .arg("max", MAX_QUERY_CHARS),
                ));
            }
            Some(q) => Some(format!("{}%", escape_like(q))),
        };
        Ok(Paging {
            prefix,
            after,
            limit,
        })
    }
}

/// `text` with the `LIKE` wildcards and the escape character escaped.
fn escape_like(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// `(created_at, id)` of a page's last row, opaque.
fn encode_cursor(at: DateTime<Utc>, id: Uuid) -> String {
    URL_SAFE_NO_PAD.encode(format!(
        "{}|{id}",
        at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
    ))
}

fn decode_cursor(cursor: &str) -> Result<(DateTime<Utc>, Uuid)> {
    let bad = || Error::BadRequest(MESSAGES.reason("invalid_cursor"));
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| bad())?;
    let text = String::from_utf8(bytes).map_err(|_| bad())?;
    let (at, id) = text.split_once('|').ok_or_else(bad)?;
    let at = DateTime::parse_from_rfc3339(at).map_err(|_| bad())?;
    Ok((at.with_timezone(&Utc), id.parse().map_err(|_| bad())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(q: Option<&str>, after: Option<&str>, limit: Option<u16>) -> AdminPage {
        AdminPage {
            q: q.map(str::to_owned),
            after: after.map(str::to_owned),
            limit,
        }
    }

    #[test]
    fn defaults_to_fifty_newest_first() {
        let paging = page(None, None, None).paging().unwrap();
        assert_eq!(paging.limit, 50);
        assert_eq!(paging.fetch(), 51);
        assert_eq!((paging.prefix, paging.after), (None, None));
    }

    #[test]
    fn cursors_round_trip_and_reject_junk() {
        let at = Utc::now();
        let id = Uuid::now_v7();
        let cursor = encode_cursor(at, id);
        assert_eq!(
            page(None, Some(&cursor), None).paging().unwrap().after,
            Some((at, id))
        );
        for junk in ["not-a-cursor", &URL_SAFE_NO_PAD.encode("2026-01-01|nope")] {
            assert!(page(None, Some(junk), None).paging().is_err(), "{junk}");
        }
    }

    #[test]
    fn limits_and_queries_are_bounded() {
        for limit in [0, 201] {
            assert!(page(None, None, Some(limit)).paging().is_err(), "{limit}");
        }
        assert_eq!(page(None, None, Some(200)).paging().unwrap().limit, 200);
        assert!(page(Some(&"a".repeat(101)), None, None).paging().is_err());
        assert_eq!(page(Some("  "), None, None).paging().unwrap().prefix, None);
    }

    #[test]
    fn search_text_is_a_literal_prefix() {
        let paging = page(Some(" 50%_off\\ "), None, None).paging().unwrap();
        assert_eq!(paging.prefix.as_deref(), Some("50\\%\\_off\\\\%"));
    }

    #[test]
    fn finish_cuts_the_page_and_points_past_it() {
        let paging = page(None, None, Some(2)).paging().unwrap();
        let at = Utc::now();
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::now_v7()).collect();
        let (rows, next) = paging.finish(ids.clone(), |id| (at, *id));
        assert_eq!(rows, ids[..2]);
        assert_eq!(decode_cursor(&next.unwrap()).unwrap(), (at, ids[1]));
        let (rows, next) = paging.finish(ids[..2].to_vec(), |id| (at, *id));
        assert_eq!((rows.len(), next), (2, None));
    }
}
