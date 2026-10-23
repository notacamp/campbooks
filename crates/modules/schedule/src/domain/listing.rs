//! The events of a member within a window: single events as stored, series
//! expanded into their occurrences, exceptions in place of the occurrences
//! they replace.

use std::collections::HashSet;

use campbooks_kernel::{Result, tenancy::UserId};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgConnection;

use super::{
    moment::{Moment, Span},
    recur::{self, Occurrence},
    rows::{EVENT_COLUMNS, EventRow},
};
use crate::model::{CalendarId, EventId};

/// What to leave out.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// Only these calendars (all the member's shown ones when `None`).
    pub calendars: Option<Vec<CalendarId>>,
    /// Only these kinds.
    pub kinds: Option<Vec<String>>,
    /// Only titles that contain this, ignoring case and accents.
    pub q: Option<String>,
    /// Only events that count when free time is worked out.
    pub busy_only: bool,
}

/// An event or an occurrence in a window.
#[derive(Debug, Clone)]
pub struct Listed {
    /// The stored event (the series, for an occurrence).
    pub row: EventRow,
    /// When it is.
    pub span: Span,
    /// The occurrence it stands for, if the row is a series.
    pub occurrence: Option<Occurrence>,
}

/// `text` as a `LIKE` pattern's literal.
fn escape_like(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The member's events that overlap `from..to`, earliest first. Only
/// calendars they see are read: their Campbooks calendar and the provider
/// calendars that sync. Cancelled, deleted and pushed-away events are left
/// out.
pub async fn list(
    conn: &mut PgConnection,
    member: UserId,
    filter: &Filter,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<Listed>> {
    let from_day = (from - TimeDelta::days(1)).date_naive();
    let to_day = to.date_naive();
    let rows = sqlx::query_as::<_, EventRow>(&format!(
        r"SELECT {EVENT_COLUMNS}
            FROM schedule.events e
            JOIN schedule.calendars c ON c.id = e.calendar_id
           WHERE c.member_id = $1
             AND (c.source = 'campbooks' OR c.syncing)
             AND ($2::uuid[] IS NULL OR c.id = ANY($2))
             AND e.deleted_at IS NULL AND e.pushed_to IS NULL
             AND e.status <> 'cancelled'
             AND ($3::text[] IS NULL OR e.kind = ANY($3))
             AND ($4::text IS NULL
                  OR public.unaccent(lower(e.title))
                     LIKE '%' || public.unaccent(lower($4)) || '%' ESCAPE '\')
             AND (NOT $9 OR e.busy)
             AND (
                  (cardinality(e.recurrence) = 0
                   AND ((NOT e.all_day AND e.starts_at < $6 AND e.ends_at > $5)
                        OR (e.all_day AND e.starts_on <= $8 AND e.ends_on > $7)))
                  OR (cardinality(e.recurrence) > 0 AND e.series_id IS NULL
                      AND ((NOT e.all_day AND e.starts_at < $6)
                           OR (e.all_day AND e.starts_on <= $8)))
             )"
    ))
    .bind(member)
    .bind(filter.calendars.as_deref())
    .bind(filter.kinds.as_deref())
    .bind(filter.q.as_deref().map(escape_like))
    .bind(from)
    .bind(to)
    .bind(from_day)
    .bind(to_day)
    .bind(filter.busy_only)
    .fetch_all(&mut *conn)
    .await?;

    let series: Vec<EventId> = rows
        .iter()
        .filter(|row| row.is_series())
        .map(|row| row.id)
        .collect();
    // The occurrences that exceptions replace (wherever the exception now is).
    let replaced: HashSet<(EventId, String)> = if series.is_empty() {
        HashSet::new()
    } else {
        sqlx::query_as::<_, (EventId, String)>(
            "SELECT series_id, original_start FROM schedule.events
              WHERE series_id = ANY($1) AND original_start IS NOT NULL AND deleted_at IS NULL",
        )
        .bind(&series)
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .collect()
    };

    let mut listed = Vec::new();
    for row in rows {
        let Some(span) = row.span() else { continue };
        if row.is_series() {
            for occurrence in
                recur::expand(span, row.time_zone.as_deref(), &row.recurrence, from, to)
            {
                if replaced.contains(&(row.id, occurrence.key.clone())) {
                    continue;
                }
                listed.push(Listed {
                    span: occurrence.span,
                    occurrence: Some(occurrence),
                    row: row.clone(),
                });
            }
        } else if span.overlaps(from, to) {
            listed.push(Listed {
                row,
                span,
                occurrence: None,
            });
        }
    }
    listed.sort_by_key(|item| (item.span.start.instant(), item.row.id));
    Ok(listed)
}

/// Whether `moment` is the start of an occurrence of `row`, a series.
pub fn is_occurrence(row: &EventRow, key: &str) -> Option<Occurrence> {
    let span = row.span()?;
    let at = Moment::parse(key)?;
    let (from, to) = (
        at.instant() - TimeDelta::seconds(1),
        at.instant() + TimeDelta::seconds(1) + span.length(),
    );
    recur::expand(span, row.time_zone.as_deref(), &row.recurrence, from, to)
        .into_iter()
        .find(|occurrence| occurrence.key == key)
}
