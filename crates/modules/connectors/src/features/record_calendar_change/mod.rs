//! `connectors.record_calendar_change` ← `schedule.outbound_requested`: a
//! member's change to an event on a provider calendar is recorded for the
//! calendar stream's next run, which makes it before it reads
//! (`contracts/schedule.outbound_requested.v1.schema.json`).
//!
//! The event is not taken as proof (campbooks-cloud#227): before anything is
//! recorded the schedule module is asked, through the kernel's
//! calendar-requests port, whether the request still stands (waiting for the
//! provider, its binding verifying, the member still allowed to change that
//! calendar), and the document recorded is the one it stored, never the one
//! the event names. Each change recorded is bound to the request and to the
//! stream and connection that make it with a MAC under a key the database
//! never holds. A change that does not stand, or cannot be made at all (the
//! connection is gone, or waits for the member), ends at once for the
//! schedule module to know: nothing is written and nothing waits for ever.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError, tenancy::WorkspaceId};
use sqlx::PgConnection;

use crate::{
    calendar::{
        ChangeDocument,
        changes::{self, MAX_CHANGE_BYTES, Request},
    },
    deps::Deps,
    domain::{
        model::{ConnectionId, Provider, StreamId},
        schedule,
    },
    events::{CalendarChangeCompleted, CalendarOutcome, CalendarReason, OutboundRequested},
};

/// Records a member's calendar changes for the connector to make.
#[derive(Debug, Clone)]
pub struct RecordCalendarChange {
    deps: Deps,
}

impl RecordCalendarChange {
    /// The consumer, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

/// The stream that reads a connection's calendars.
struct Target {
    stream: StreamId,
    provider: Provider,
    paused: bool,
}

impl EventConsumer for RecordCalendarChange {
    type Event = OutboundRequested;
    const NAME: &'static str = "connectors.record_calendar_change";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<OutboundRequested>,
    ) -> Result<(), JobError> {
        let asked = event.payload;
        let workspace: WorkspaceId = asked
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("outbound_requested names no workspace"))?;
        let now = self.deps.clock.now();
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let stored = state
            .calendar_requests
            .stored_change(&state, tx, workspace, asked.request)
            .await?;
        let end = |reason: CalendarReason| CalendarChangeCompleted {
            workspace: asked.workspace.clone(),
            request: asked.request,
            calendar: asked.calendar,
            event: asked.event,
            outcome: CalendarOutcome::Skipped,
            provider_event_id: None,
            etag: None,
            reason: Some(reason),
        };
        let Some(stored) = stored else {
            // It does not stand: nothing is made, and the schedule module
            // is told so its event does not wait.
            tracing::warn!(%workspace, request = %asked.request, "a calendar change that does not stand is not recorded");
            state
                .events
                .publish(tx, &end(CalendarReason::Unverified))
                .await?;
            return Ok(());
        };
        if stored.calendar != asked.calendar
            || stored.event != asked.event
            || stored.connection != uuid::Uuid::from(asked.connection)
        {
            tracing::warn!(%workspace, request = %asked.request, "outbound_requested names another calendar, event or connection than the request it cites; nothing is recorded");
            return Ok(());
        }
        // The kind of change is the document's, not the event's.
        let document = changes::read_json::<ChangeDocument>(
            &state.blobs,
            workspace,
            &stored.body,
            MAX_CHANGE_BYTES,
        )
        .await?;
        let Some(document) = document.filter(|d| d.is_sound() && d.change == asked.change) else {
            state
                .events
                .publish(tx, &end(CalendarReason::Rejected))
                .await?;
            return Ok(());
        };
        let Some(target) = target_of(tx, asked.connection).await? else {
            state
                .events
                .publish(tx, &end(CalendarReason::NotConnected))
                .await?;
            return Ok(());
        };
        let offered = match target.provider {
            Provider::Google => self.deps.google.is_some(),
            Provider::Zoho => self.deps.zoho.is_some() && self.deps.config.zoho_calendar,
            Provider::Imap => false,
        };
        if !offered {
            state
                .events
                .publish(tx, &end(CalendarReason::NotConnected))
                .await?;
            return Ok(());
        }
        if target.paused {
            // The member must connect again first.
            state
                .events
                .publish(tx, &end(CalendarReason::NeedsReauth))
                .await?;
            return Ok(());
        }
        // Serialise with the stream's run, which looks for changes recorded
        // while it ran under the same lock.
        sqlx::query("SELECT id FROM connectors.streams WHERE id = $1 FOR UPDATE")
            .bind(target.stream)
            .fetch_one(&mut **tx)
            .await?;
        let recorded = changes::record(
            tx,
            &state.binding,
            workspace,
            (target.stream, asked.connection),
            &Request {
                request: asked.request,
                calendar: stored.calendar,
                event: stored.event,
                change: &document.change,
                body: &stored.body,
            },
        )
        .await?;
        if recorded.is_some() {
            schedule::run_now(tx, workspace, target.stream, now).await?;
            tracing::info!(%workspace, stream = %target.stream, "a calendar change was recorded");
        }
        Ok(())
    }
}

/// The calendar stream of `connection`, when it has one.
async fn target_of(
    conn: &mut PgConnection,
    connection: ConnectionId,
) -> Result<Option<Target>, JobError> {
    let row: Option<(StreamId, String, bool)> = sqlx::query_as(
        "SELECT s.id, c.provider, s.paused
           FROM connectors.streams s
           JOIN connectors.connections c ON c.id = s.connection_id
          WHERE s.connection_id = $1 AND s.capability = 'calendar'",
    )
    .bind(connection)
    .fetch_optional(conn)
    .await?;
    Ok(row
        .map(|(stream, provider, paused)| {
            Provider::parse(&provider).map(|provider| Target {
                stream,
                provider,
                paused,
            })
        })
        .transpose()?)
}
