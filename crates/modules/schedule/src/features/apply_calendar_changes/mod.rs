//! `schedule.apply_calendar_changes` ← `connectors.calendar_changed`: one
//! batch of changes to one provider calendar, applied in order. An upsert
//! makes or changes the event with the provider's id and etag; a removal
//! cancels it. A recurring series arrives once, with the provider's own
//! recurrence lines, and each exception as an event of its own naming its
//! series and the occurrence it replaces. A batch for a calendar that does
//! not sync is ignored, and an event with a member's change still waiting
//! for the provider keeps that change.

use std::collections::HashSet;

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError,
    tenancy::{UserId, WorkspaceId},
};
use sqlx::{PgConnection, Postgres, Transaction};

use crate::{
    domain::{
        blobs::{MAX_DOCUMENT_BYTES, read_json},
        provider::{self, Parsed, Snapshot},
        publish,
        rows::{CALENDAR_COLUMNS, CalendarRow, EVENT_COLUMNS, EventRow},
        tracked,
    },
    inbound::{BatchEvent, CalendarBatch, CalendarChanged},
    model::{EventId, EventKind},
    tracking,
};

/// Applies the changes of a calendar batch.
pub struct ApplyCalendarChanges;

impl EventHandler for ApplyCalendarChanges {
    type Event = CalendarChanged;
    const NAME: &'static str = "schedule.apply_calendar_changes";
    const QUEUE: &'static str = "sync";

    async fn handle(ctx: &mut JobContext, event: Event<CalendarChanged>) -> Result<(), JobError> {
        let changed = event.payload;
        let workspace: WorkspaceId = changed
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("calendar_changed names an invalid workspace"))?;
        let state = ctx.state().clone();
        let batch: CalendarBatch = read_json(
            &state.blobs,
            &changed.workspace,
            &changed.batch,
            MAX_DOCUMENT_BYTES,
        )
        .await?;
        if batch.version != 1 {
            return Err(JobError::fatal(
                "the calendar batch is of an unsupported version",
            ));
        }
        let tx = ctx.tx().await?;
        let calendar = sqlx::query_as::<_, CalendarRow>(sqlx::AssertSqlSafe(format!(
            "SELECT {CALENDAR_COLUMNS} FROM schedule.calendars
              WHERE connection_id = $1 AND provider_calendar_id = $2 AND source = 'provider'
                FOR SHARE"
        )))
        .bind(changed.connection)
        .bind(&batch.provider_calendar_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(calendar) = calendar else {
            // The grant that makes the calendar may not have been applied yet.
            return Err(JobError::retry("the calendar of a batch is not known yet"));
        };
        if !calendar.syncing {
            return Ok(());
        }
        let owner = calendar.member_id;
        let mut applied = Applied::default();
        // Series first, then the exceptions that name them.
        for exceptions in [false, true] {
            for upsert in &batch.upserts {
                if upsert.series.is_some() == exceptions {
                    upsert_event(
                        &state,
                        tx,
                        workspace,
                        &calendar,
                        owner,
                        upsert,
                        &mut applied,
                    )
                    .await?;
                }
            }
        }
        for removal in &batch.removals {
            if remove_event(
                &state,
                tx,
                workspace,
                &calendar,
                owner,
                &removal.provider_id,
            )
            .await?
            {
                applied.cancelled += 1;
            }
        }
        if batch.full {
            let kept: HashSet<&str> = batch
                .upserts
                .iter()
                .map(|upsert| upsert.provider_id.as_str())
                .collect();
            let gone = sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
                "SELECT {EVENT_COLUMNS} FROM schedule.events e
                  WHERE e.calendar_id = $1 AND e.provider_event_id IS NOT NULL
                    AND e.status <> 'cancelled' AND e.deleted_at IS NULL
                    AND e.outbound_pending IS NULL"
            )))
            .bind(calendar.id)
            .fetch_all(&mut **tx)
            .await?;
            for row in gone {
                let id = row.provider_event_id.as_deref().unwrap_or_default();
                if !kept.contains(id) && cancel(&state, tx, workspace, owner, &row).await? {
                    applied.cancelled += 1;
                }
            }
        }
        tracing::info!(
            created = applied.created,
            changed = applied.changed,
            cancelled = applied.cancelled,
            "calendar batch applied"
        );
        applied
            .track(&state, tx, workspace, &calendar, batch.full)
            .await?;
        Ok(())
    }
}

/// What a batch did, for the log and the record.
#[derive(Default)]
struct Applied {
    created: u32,
    changed: u32,
    cancelled: u32,
}

impl Applied {
    /// Track, in `tx`, one summary of the batch for `calendar`
    /// (`schedule.provider_changes_applied`), when it did anything.
    async fn track(
        &self,
        state: &AppState,
        tx: &mut Transaction<'static, Postgres>,
        workspace: WorkspaceId,
        calendar: &CalendarRow,
        full: bool,
    ) -> Result<(), JobError> {
        if self.created + self.changed + self.cancelled == 0 {
            return Ok(());
        }
        let Some(snapshot) = tracked::calendar(tx, calendar.id).await? else {
            return Ok(());
        };
        state
            .tracker
            .track(
                tx,
                &tracking::ProviderChangesApplied {
                    workspace,
                    calendar: snapshot,
                    created: i64::from(self.created),
                    changed: i64::from(self.changed),
                    cancelled: i64::from(self.cancelled),
                    full,
                },
            )
            .await?;
        Ok(())
    }
}

/// The event of `calendar` with the provider id `provider_id`.
async fn by_provider_id(
    conn: &mut PgConnection,
    calendar: &CalendarRow,
    provider_id: &str,
) -> Result<Option<EventRow>, sqlx::Error> {
    sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {EVENT_COLUMNS} FROM schedule.events e
          WHERE e.calendar_id = $1 AND e.provider_event_id = $2
            FOR UPDATE"
    )))
    .bind(calendar.id)
    .bind(provider_id)
    .fetch_optional(conn)
    .await
}

/// An event asked of the provider whose id the provider now gives: the event
/// Campbooks created there (by the id it wrote), or the exception it asked
/// for (by the occurrence it replaces).
async fn adopt(
    conn: &mut PgConnection,
    calendar: &CalendarRow,
    upsert: &BatchEvent,
    series: Option<&EventRow>,
    parsed: &Parsed,
) -> Result<Option<EventRow>, sqlx::Error> {
    let written = upsert
        .private
        .as_ref()
        .and_then(|private| private.campbooks_event.as_deref())
        .and_then(|id| id.parse::<EventId>().ok());
    if let Some(id) = written {
        let row = sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
            "SELECT {EVENT_COLUMNS} FROM schedule.events e
              WHERE e.id = $1 AND e.calendar_id = $2 AND e.provider_event_id IS NULL
                FOR UPDATE"
        )))
        .bind(id)
        .bind(calendar.id)
        .fetch_optional(&mut *conn)
        .await?;
        if row.is_some() {
            return Ok(row);
        }
    }
    if let (Some(series), Some(original)) = (series, &parsed.original) {
        return sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
            "SELECT {EVENT_COLUMNS} FROM schedule.events e
              WHERE e.series_id = $1 AND e.original_start = $2 AND e.provider_event_id IS NULL
                FOR UPDATE"
        )))
        .bind(series.id)
        .bind(original)
        .fetch_optional(conn)
        .await;
    }
    Ok(None)
}

/// The record a provider event says it was made from, if the line is one.
fn source_of(upsert: &BatchEvent) -> Option<(String, String)> {
    let text = upsert.private.as_ref()?.campbooks_source.as_deref()?;
    let (kind, id) = text.split_once(':')?;
    let well_formed = !id.is_empty()
        && kind.split_once('.').is_some_and(|(module, record)| {
            !module.is_empty()
                && !record.is_empty()
                && kind
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c == '.')
        });
    well_formed.then(|| (kind.to_owned(), id.to_owned()))
}

#[allow(clippy::too_many_lines)]
async fn upsert_event(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    calendar: &CalendarRow,
    owner: UserId,
    upsert: &BatchEvent,
    applied: &mut Applied,
) -> Result<(), JobError> {
    let Some(parsed) = provider::parse(upsert) else {
        tracing::warn!("an event of a calendar batch has times that do not read; skipped");
        return Ok(());
    };
    // An exception names its series.
    let series = match &upsert.series {
        Some(provider_id) => {
            let found = by_provider_id(tx, calendar, provider_id).await?;
            if found.is_none() {
                tracing::warn!("an exception of a calendar batch names a series not stored");
                return Ok(());
            }
            found
        }
        None => None,
    };
    let mut existing = by_provider_id(tx, calendar, &upsert.provider_id).await?;
    let mut adopted = false;
    if existing.is_none() {
        existing = adopt(tx, calendar, upsert, series.as_ref(), &parsed).await?;
        adopted = existing.is_some();
    }
    let snapshot = Snapshot::from_parsed(&parsed);
    let Some(row) = existing else {
        if snapshot.is_cancelled() {
            return Ok(());
        }
        let kind = parsed
            .kind
            .or_else(|| series.as_ref().and_then(|s| s.kind.parse().ok()))
            .unwrap_or(EventKind::None);
        let (starts_at, ends_at, starts_on, ends_on) = parsed.span.columns();
        let source = source_of(upsert);
        let id = EventId::new();
        sqlx::query(
            "INSERT INTO schedule.events
                 (id, workspace_id, calendar_id, kind, title, starts_at, ends_at, starts_on,
                  ends_on, all_day, time_zone, place, notes, status, provider_event_id, etag,
                  recurrence, series_id, original_start, source_type, source_id, origin,
                  ical_uid, response, provider_version)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                     $18, $19, $20, $21, 'provider', $22, $23, $24)",
        )
        .bind(id)
        .bind(workspace)
        .bind(calendar.id)
        .bind(kind.as_str())
        .bind(&parsed.title)
        .bind(starts_at)
        .bind(ends_at)
        .bind(starts_on)
        .bind(ends_on)
        .bind(parsed.span.start.is_day())
        .bind(&parsed.time_zone)
        .bind(&parsed.place)
        .bind(&parsed.notes)
        .bind(&parsed.status)
        .bind(&upsert.provider_id)
        .bind(&upsert.etag)
        .bind(&parsed.recurrence)
        .bind(series.as_ref().map(|s| s.id))
        .bind(&parsed.original)
        .bind(source.as_ref().map(|(kind, _)| kind.as_str()))
        .bind(source.as_ref().map(|(_, id)| id.as_str()))
        .bind(&upsert.ical_uid)
        .bind(&upsert.response)
        .bind(snapshot.to_json())
        .execute(&mut **tx)
        .await?;
        let row = crate::domain::rows::event_by_id(tx, id).await?;
        if let Some(row) = row {
            publish::created(state, tx, workspace, owner, &row).await?;
        }
        applied.created += 1;
        return Ok(());
    };
    // A member's change waits for the provider (or they deleted the event):
    // theirs is kept, and the provider's version arrives with the outcome.
    if row.deleted_at.is_some() || (row.is_pending() && !adopted) {
        return Ok(());
    }
    if adopted {
        // Campbooks created it; the provider's id and etag are now known.
        sqlx::query(
            "UPDATE schedule.events SET provider_event_id = $2, etag = $3, updated_at = now()
              WHERE id = $1",
        )
        .bind(row.id)
        .bind(&upsert.provider_id)
        .bind(&upsert.etag)
        .execute(&mut **tx)
        .await?;
        if row.is_pending() {
            return Ok(());
        }
    }
    let before = Snapshot::of(&row);
    snapshot.restore(tx, row.id).await?;
    let kind = if row.kind == "none" {
        parsed.kind.map_or("none", EventKind::as_str)
    } else {
        row.kind.as_str()
    };
    sqlx::query(
        "UPDATE schedule.events
            SET etag = $2, ical_uid = coalesce($3, ical_uid), response = coalesce($4, response),
                kind = $5, provider_version = $6, notice = NULL, updated_at = now()
          WHERE id = $1",
    )
    .bind(row.id)
    .bind(&upsert.etag)
    .bind(&upsert.ical_uid)
    .bind(&upsert.response)
    .bind(kind)
    .bind(snapshot.to_json())
    .execute(&mut **tx)
    .await?;
    if before != snapshot || kind != row.kind {
        let now = crate::domain::rows::event_by_id(tx, row.id).await?;
        if let Some(now) = now {
            if snapshot.is_cancelled() && !before.is_cancelled() {
                publish::cancelled(state, tx, workspace, owner, &now).await?;
                applied.cancelled += 1;
            } else {
                publish::changed(state, tx, workspace, owner, &now).await?;
                applied.changed += 1;
            }
        }
    }
    Ok(())
}

/// The provider deleted `provider_id`: the event is cancelled (an event the
/// member deleted is gone for good). Whether an event was cancelled.
async fn remove_event(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    calendar: &CalendarRow,
    owner: UserId,
    provider_id: &str,
) -> Result<bool, JobError> {
    let Some(row) = by_provider_id(tx, calendar, provider_id).await? else {
        return Ok(false);
    };
    if row.deleted_at.is_some() {
        // Deleted by the member, and now at the provider too: the row only
        // stays so the delete can be undone by making the event again.
        sqlx::query(
            "UPDATE schedule.events
                SET provider_event_id = NULL, etag = NULL, outbound_pending = NULL
              WHERE id = $1",
        )
        .bind(row.id)
        .execute(&mut **tx)
        .await?;
        return Ok(false);
    }
    cancel(state, tx, workspace, owner, &row).await
}

/// Mark `row` (and the exceptions of a series) cancelled, once. Whether it
/// was cancelled now.
async fn cancel(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    owner: UserId,
    row: &EventRow,
) -> Result<bool, JobError> {
    if row.status == "cancelled" {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE schedule.events SET status = 'cancelled', updated_at = now()
          WHERE id = $1 OR series_id = $1",
    )
    .bind(row.id)
    .execute(&mut **tx)
    .await?;
    publish::cancelled(state, tx, workspace, owner, row).await?;
    Ok(true)
}
