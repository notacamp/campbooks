//! Cursor paging for the admin lists: `?after=<opaque>&limit=` (default 50,
//! at most 200). A cursor is the sort key of the last row returned, so a
//! page never skips or repeats a row that was there from the start.

use std::{fmt::Display, str::FromStr};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{Error, Result};
use chrono::{DateTime, Utc};

use crate::MESSAGES;

/// Most rows one call returns.
pub const MAX_LIMIT: u16 = 200;

/// Rows per call unless asked otherwise.
pub const DEFAULT_LIMIT: u16 = 50;

/// The validated `limit`.
pub fn limit(requested: Option<u16>) -> Result<u16> {
    let limit = requested.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(Error::BadRequest(
            MESSAGES.reason("invalid_limit").arg("max", MAX_LIMIT),
        ));
    }
    Ok(limit)
}

/// Where the next page starts: after the row with this sort key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor<I> {
    /// The row's rank (lower first; 0 when a list has no ranking).
    pub rank: i32,
    /// The row's timestamp (newer first within a rank).
    pub at: DateTime<Utc>,
    /// The row's id (the tie-break, larger first).
    pub id: I,
}

impl<I: Display + FromStr> Cursor<I> {
    /// The opaque string for `?after=`.
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(format!(
            "{}.{}.{}",
            self.rank,
            self.at.timestamp_micros(),
            self.id
        ))
    }

    /// The cursor `raw` encodes; a 400 when it is not one.
    pub fn decode(raw: &str) -> Result<Self> {
        let invalid = || Error::BadRequest(MESSAGES.reason("invalid_cursor"));
        let bytes = URL_SAFE_NO_PAD.decode(raw).map_err(|_| invalid())?;
        let text = String::from_utf8(bytes).map_err(|_| invalid())?;
        let mut parts = text.splitn(3, '.');
        let (Some(rank), Some(micros), Some(id)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid());
        };
        Ok(Self {
            rank: rank.parse().map_err(|_| invalid())?,
            at: micros
                .parse()
                .ok()
                .and_then(DateTime::from_timestamp_micros)
                .ok_or_else(invalid)?,
            id: id.parse().map_err(|_| invalid())?,
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::tenancy::UserId;
    use chrono::{DateTime, Utc};

    use super::{Cursor, limit};

    #[test]
    fn a_cursor_round_trips_and_garbage_is_refused() {
        let cursor = Cursor {
            rank: 1,
            // Postgres keeps microseconds.
            at: DateTime::from_timestamp_micros(Utc::now().timestamp_micros()).unwrap(),
            id: UserId::new(),
        };
        assert_eq!(Cursor::<UserId>::decode(&cursor.encode()).unwrap(), cursor);
        for garbage in ["", "not base64!", "MS4y", "YS5iLmM"] {
            assert!(Cursor::<UserId>::decode(garbage).is_err(), "{garbage}");
        }
    }

    #[test]
    fn limits_are_bounded() {
        assert_eq!(limit(None).unwrap(), 50);
        assert_eq!(limit(Some(200)).unwrap(), 200);
        assert!(limit(Some(0)).is_err());
        assert!(limit(Some(201)).is_err());
    }
}
