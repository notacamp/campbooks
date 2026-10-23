//! `connectors.apply_calendar_sync` ← `schedule.calendar_sync_changed`: a
//! member turned a provider calendar on or off. A calendar turned on is read
//! from the start (its sync token is dropped, so the first read is a full
//! one); a calendar turned off is not read. Deliveries come in any order
//! (and again): the newest event the calendar heard wins, and an older or
//! repeated one changes nothing.
//!
//! The event names the schedule module's calendar and the connection; which
//! calendar of the provider it is travels in a small blob (the provider's
//! calendar id can be an address, so never in an event). Turning a calendar
//! on only makes the connector read what the member's own grant already
//! reaches, and the schedule module ignores a batch for a calendar that does
//! not sync.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError, tenancy::WorkspaceId};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    calendar::changes,
    deps::Deps,
    domain::{model::StreamId, schedule},
    events::CalendarSyncChanged,
};

/// The most bytes of a `calendar-ref` document.
const MAX_REF_BYTES: usize = 4 * 1024;

/// A `calendar-ref` v1 document: which calendar of the provider.
#[derive(Debug, Deserialize)]
struct CalendarRef {
    version: u32,
    provider_calendar_id: String,
}

/// Turns the reading of a provider calendar on and off.
#[derive(Debug, Clone)]
pub struct ApplyCalendarSync {
    deps: Deps,
}

impl ApplyCalendarSync {
    /// The consumer, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

impl EventConsumer for ApplyCalendarSync {
    type Event = CalendarSyncChanged;
    const NAME: &'static str = "connectors.apply_calendar_sync";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<CalendarSyncChanged>,
    ) -> Result<(), JobError> {
        let heard = *event.id.as_uuid();
        let changed = event.payload;
        let workspace: WorkspaceId = changed
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("calendar_sync_changed names no workspace"))?;
        let Some(reference) = &changed.provider_calendar else {
            tracing::warn!(%workspace, "a calendar sync change names no provider calendar");
            return Ok(());
        };
        let state = ctx.state().clone();
        let Some(document) =
            changes::read_json::<CalendarRef>(&state.blobs, workspace, reference, MAX_REF_BYTES)
                .await?
        else {
            tracing::warn!(%workspace, "a calendar reference could not be read");
            return Ok(());
        };
        if document.version != 1
            || document.provider_calendar_id.is_empty()
            || document.provider_calendar_id.chars().count() > 512
        {
            return Ok(());
        }
        let now = self.deps.clock.now();
        let tx = ctx.tx().await?;
        let stream: Option<StreamId> = sqlx::query_scalar(
            "SELECT id FROM connectors.streams
              WHERE connection_id = $1 AND capability = 'calendar' FOR UPDATE",
        )
        .bind(changed.connection)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(stream) = stream else {
            return Ok(());
        };
        // Only an event newer than the one the calendar last heard applies
        // (event ids are UUIDv7, time-ordered). Turned on: read from the
        // start (the token goes); a repeated delivery is not newer, so it
        // never drops the token again.
        let applied = sqlx::query(
            "INSERT INTO connectors.calendars
                 (id, workspace_id, stream_id, provider_calendar_id, schedule_calendar, syncing,
                  sync_changed_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (stream_id, provider_calendar_id) DO UPDATE
                SET schedule_calendar = EXCLUDED.schedule_calendar,
                    sync_token = CASE WHEN EXCLUDED.syncing THEN NULL
                                      ELSE connectors.calendars.sync_token END,
                    syncing = EXCLUDED.syncing,
                    sync_changed_by = EXCLUDED.sync_changed_by
              WHERE connectors.calendars.sync_changed_by IS NULL
                 OR connectors.calendars.sync_changed_by < EXCLUDED.sync_changed_by",
        )
        .bind(Uuid::now_v7())
        .bind(workspace)
        .bind(stream)
        .bind(&document.provider_calendar_id)
        .bind(changed.calendar)
        .bind(changed.syncing)
        .bind(heard)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if applied == 1 && changed.syncing {
            schedule::run_now(tx, workspace, stream, now).await?;
        }
        Ok(())
    }
}
