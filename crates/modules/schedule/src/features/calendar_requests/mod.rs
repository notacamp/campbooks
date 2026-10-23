//! The kernel's calendar-requests port: before connectors writes a change at
//! a provider calendar because this module asked for it, it asks whether the
//! request still stands. Any transaction working in a workspace can append
//! that workspace's `schedule.outbound_requested`, so the event alone proves
//! nothing.
//!
//! A request stands while it waits for the provider, its binding verifies
//! (`domain::binding`: the request that wrote it held the key), and the
//! member who asked still has that calendar and may write it.

use std::sync::Arc;

use campbooks_kernel::{
    AppState,
    blobs::BlobRef,
    outbound::{CalendarPort, CalendarRequests, StoredCalendarChange, StoredCalendarChangeFuture},
    tenancy::{UserId, WorkspaceId},
};
use sqlx::{
    PgConnection,
    types::{Json, Uuid},
};

use crate::{
    domain::binding::ChangeRequest,
    model::{CalendarId, ConnectionId, EventId, RequestId},
};

#[derive(Debug, Clone, Copy)]
struct StoredChanges;

/// The port, for the server to register.
pub fn calendar_requests() -> CalendarPort {
    Arc::new(StoredChanges)
}

type Row = (
    UserId,
    CalendarId,
    EventId,
    ConnectionId,
    String,
    Json<BlobRef>,
    Option<Vec<u8>>,
);

impl CalendarRequests for StoredChanges {
    fn stored_change<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        request: Uuid,
    ) -> StoredCalendarChangeFuture<'a> {
        Box::pin(async move {
            let request = RequestId::from(request);
            let Some((member, calendar, event, connection, change, body, mac)) =
                sqlx::query_as::<_, Row>(
                    "SELECT member_id, calendar_id, event_id, connection_id, change, body, mac
                       FROM schedule.change_requests
                      WHERE id = $1 AND state = 'waiting'
                        FOR SHARE",
                )
                .bind(request)
                .fetch_optional(&mut *conn)
                .await?
            else {
                return Ok(None);
            };
            let body = body.0;
            let bound = ChangeRequest {
                workspace,
                request,
                member,
                calendar,
                event,
                connection,
                change: &change,
                body: &body,
            };
            if !bound.holds(&state.binding, mac.as_deref()) {
                tracing::warn!(
                    %workspace,
                    %request,
                    "a recorded calendar change is not bound to the request that made it; \
                     it is not made"
                );
                return Ok(None);
            }
            // The member still has the calendar, and it can still be written.
            let usable: bool = sqlx::query_scalar(
                "SELECT EXISTS (
                     SELECT 1 FROM schedule.calendars
                      WHERE id = $1 AND member_id = $2 AND connection_id = $3
                        AND writable AND syncing)",
            )
            .bind(calendar)
            .bind(member)
            .bind(connection)
            .fetch_one(&mut *conn)
            .await?;
            if !usable {
                return Ok(None);
            }
            Ok(Some(StoredCalendarChange {
                calendar: *calendar.as_uuid(),
                event: *event.as_uuid(),
                connection: *connection.as_uuid(),
                body,
            }))
        })
    }
}
