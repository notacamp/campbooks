//! A member's change to an event on a provider calendar, asked of connectors:
//! the `calendar-change` document goes to a blob, the request is recorded and
//! bound (campbooks-cloud#227), the event is marked pending, and
//! `schedule.outbound_requested` is published, all in the member's
//! transaction. Connectors asks the kernel's calendar-requests port
//! (`features::calendar_requests`) whether the request stands before it
//! writes anything.

use campbooks_kernel::{
    AppState, Error, Result,
    blobs::BlobOwner,
    tenancy::{UserId, WorkspaceId},
};
use sqlx::{Postgres, Transaction, types::Json};

use super::{
    binding::ChangeRequest,
    rows::{CalendarRow, EventRow},
};
use crate::{
    events::OutboundRequested,
    inbound::{ChangeDocument, ChangeEvent, PrivateProperties},
    model::{CalendarChange, RequestId},
};

/// The media type of a `calendar-change` v1 document.
pub const CONTENT_TYPE: &str = "application/vnd.campbooks.calendar-change+json; version=1";
/// The record type that holds a request's blob.
pub const BLOB_OWNER: &str = "schedule.change_request";

/// What to write for `row`: its text and times, and for a create what makes
/// it come back with its kind and source.
fn change_event(row: &EventRow, creating: bool) -> Result<ChangeEvent> {
    let span = row.span().ok_or(Error::NotFound)?;
    let zone = row.time_zone.as_deref();
    Ok(ChangeEvent {
        title: Some(row.title.clone()),
        place: row.place.clone(),
        description: row.notes.clone(),
        start: span.start.to_provider(zone),
        end: Some(span.end.to_provider(zone)),
        private: creating.then(|| PrivateProperties {
            campbooks_event: Some(row.id.to_string()),
            campbooks_kind: Some(row.kind.clone()),
            campbooks_source: row
                .source()
                .map(|source| format!("{}:{}", source.record_type, source.id))
                .filter(|text| text.len() <= 200),
        }),
    })
}

/// The `calendar-change` v1 document for `change` of `row` on `calendar`.
/// `series` is the series an `update_occurrence` changes one occurrence of;
/// `response` is the answer a `respond` gives.
pub fn document(
    change: CalendarChange,
    calendar: &CalendarRow,
    row: &EventRow,
    series: Option<&EventRow>,
    response: Option<&str>,
) -> Result<ChangeDocument> {
    let provider_calendar_id = calendar
        .provider_calendar_id
        .clone()
        .ok_or(Error::NotFound)?;
    let pending = || Error::Conflict(crate::MESSAGES.reason("change_pending"));
    let mut doc = ChangeDocument {
        version: 1,
        change: change.as_str().to_owned(),
        provider_calendar_id,
        provider_event_id: None,
        etag: None,
        occurrence: None,
        response: None,
        event: None,
    };
    match change {
        CalendarChange::Create => doc.event = Some(change_event(row, true)?),
        CalendarChange::Update => {
            doc.provider_event_id = Some(row.provider_event_id.clone().ok_or_else(pending)?);
            doc.etag = Some(row.etag.clone().ok_or_else(pending)?);
            doc.event = Some(change_event(row, false)?);
        }
        CalendarChange::UpdateOccurrence => {
            let series = series.ok_or(Error::NotFound)?;
            doc.provider_event_id = Some(series.provider_event_id.clone().ok_or_else(pending)?);
            doc.etag = Some(series.etag.clone().ok_or_else(pending)?);
            doc.occurrence = row
                .original_start
                .as_deref()
                .and_then(super::moment::Moment::parse)
                .map(|moment| moment.to_provider(series.time_zone.as_deref()));
            if doc.occurrence.is_none() {
                return Err(Error::NotFound);
            }
            doc.event = Some(change_event(row, false)?);
        }
        CalendarChange::Delete => {
            doc.provider_event_id = Some(row.provider_event_id.clone().ok_or_else(pending)?);
            doc.etag = Some(row.etag.clone().ok_or_else(pending)?);
        }
        CalendarChange::Respond => {
            doc.provider_event_id = Some(row.provider_event_id.clone().ok_or_else(pending)?);
            doc.response = response.map(str::to_owned);
        }
    }
    Ok(doc)
}

/// A change to ask of connectors.
pub struct Asked<'a> {
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The member who asked.
    pub member: UserId,
    /// The provider calendar.
    pub calendar: &'a CalendarRow,
    /// The event changed.
    pub event: &'a EventRow,
    /// The kind of change.
    pub change: CalendarChange,
    /// The document to write.
    pub document: ChangeDocument,
}

/// Record `asked`, mark the event pending and publish the request.
pub async fn request(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    asked: Asked<'_>,
) -> Result<RequestId> {
    let connection = asked.calendar.connection_id.ok_or(Error::NotFound)?;
    let ws = asked.workspace.to_string();
    let bytes = serde_json::to_vec(&asked.document).map_err(Error::internal)?;
    let body = state.blobs.put(&ws, bytes, CONTENT_TYPE).await?;
    let request = RequestId::new();
    state
        .blobs
        .attach(tx, &ws, &body, &BlobOwner::new(BLOB_OWNER, request))
        .await?;
    let mac = ChangeRequest {
        workspace: asked.workspace,
        request,
        member: asked.member,
        calendar: asked.calendar.id,
        event: asked.event.id,
        connection,
        change: asked.change.as_str(),
        body: &body,
    }
    .sign(&state.binding);
    sqlx::query(
        "INSERT INTO schedule.change_requests
             (id, workspace_id, member_id, calendar_id, event_id, connection_id, change, body, mac)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(request)
    .bind(asked.workspace)
    .bind(asked.member)
    .bind(asked.calendar.id)
    .bind(asked.event.id)
    .bind(connection)
    .bind(asked.change.as_str())
    .bind(Json(&body))
    .bind(mac)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE schedule.events SET outbound_pending = $2, updated_at = now() WHERE id = $1",
    )
    .bind(asked.event.id)
    .bind(asked.change.as_str())
    .execute(&mut **tx)
    .await?;
    state
        .events
        .publish(
            tx,
            &OutboundRequested {
                workspace: ws,
                request,
                calendar: asked.calendar.id,
                event: asked.event.id,
                connection,
                change: asked.change,
                body,
            },
        )
        .await?;
    Ok(request)
}
