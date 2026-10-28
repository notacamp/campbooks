//! Invitations awaiting the member's answer (campbooks-cloud#360).
//!
//! An event is an invitation that awaits an answer while it is on one of the
//! member's provider calendars that syncs, the member was asked
//! (`response = needs_action`) and did not organise it, it is a single event
//! or a series (not one occurrence of a series, whose answer is the
//! series'), and it is not cancelled, deleted or started (an all-day event:
//! its first day is not over). Every change that can make an event one, or
//! stop it being one, calls [`announce`] in its transaction, which publishes
//! `schedule.invitation_changed` with a newer revision when the event is one
//! now or was one before. The attention module projects it into a Decide
//! item. Facts only: who asked is told by the organiser's address alone, and
//! no title, place or address leaves the module.

use campbooks_ai::decide::signals::email::{Envelope, Party, is_machine_sender};
use campbooks_kernel::{AppState, Result, tenancy::WorkspaceId};
use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone as _, Utc};
use chrono_tz::Tz;
use sqlx::{Postgres, Transaction};

use super::{
    moment::Moment,
    rows::{CALENDAR_COLUMNS, CalendarRow, EVENT_COLUMNS, EventRow, event_by_id},
    settings,
};
use crate::{
    events::{InvitationChanged, InvitationFacts},
    model::{EventId, InvitationClosed, Response},
};

/// Whether a person rather than a machine organises an event, from the
/// organiser's address alone (a no-reply or service address is a machine);
/// `None` without an address.
pub fn organizer_is_person(address: Option<&str>) -> Option<bool> {
    let address = address.map(str::trim).filter(|a| !a.is_empty())?;
    let envelope = Envelope {
        from: Party {
            name: String::new(),
            address: address.to_owned(),
        },
        ..Envelope::default()
    };
    Some(!is_machine_sender(&envelope))
}

/// When an invitation for `row` stops awaiting an answer, in `zone`: the
/// event's start, or for an all-day event the end of its first day. With
/// the day it starts on. `None` when the row holds no time.
pub fn lapse(row: &EventRow, zone: Tz) -> Option<(NaiveDate, DateTime<Utc>)> {
    let span = row.span()?;
    match span.start {
        Moment::At(at) => Some((at.with_timezone(&zone).date_naive(), at)),
        Moment::On(day) => {
            let next = day.succ_opt()?;
            let midnight = zone
                .from_local_datetime(&next.and_time(NaiveTime::MIN))
                .earliest()?
                .with_timezone(&Utc);
            Some((day, midnight))
        }
    }
}

/// Whether `row`, on `calendar`, awaits the member's answer at `now` in
/// their `zone`: its facts, or why it does not.
pub fn standing(
    row: &EventRow,
    calendar: &CalendarRow,
    zone: Tz,
    now: DateTime<Utc>,
) -> std::result::Result<InvitationFacts, InvitationClosed> {
    if row.response.as_deref() != Some(Response::NeedsAction.as_str()) || row.organizer_self {
        return Err(InvitationClosed::Answered);
    }
    if row.status == "cancelled" {
        return Err(InvitationClosed::Cancelled);
    }
    if row.deleted_at.is_some() || row.pushed_to.is_some() {
        return Err(InvitationClosed::Removed);
    }
    if calendar.is_campbooks() || !calendar.syncing || calendar.following || row.series_id.is_some()
    {
        return Err(InvitationClosed::Hidden);
    }
    let Some((on, lapses_at)) = lapse(row, zone) else {
        return Err(InvitationClosed::Removed);
    };
    if lapses_at <= now {
        return Err(InvitationClosed::Started);
    }
    Ok(InvitationFacts {
        on,
        lapses_at,
        from_person: row.organizer_person,
    })
}

/// Whether `row` could be an invitation, or was announced as one: anything
/// else is left alone without a query.
pub fn concerns(row: &EventRow) -> bool {
    row.invitation_open || row.response.as_deref() == Some(Response::NeedsAction.as_str())
}

/// Announce event `id`'s invitation in `tx`, as it stands now: published
/// when it awaits an answer, or when it was announced as awaiting one
/// before (it closed). Nothing for an event that never was one.
pub async fn announce(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    id: EventId,
) -> Result<()> {
    let Some(row) = event_by_id(tx, id).await? else {
        return Ok(());
    };
    if !concerns(&row) {
        return Ok(());
    }
    let calendar = sqlx::query_as::<_, CalendarRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {CALENDAR_COLUMNS} FROM schedule.calendars WHERE id = $1"
    )))
    .bind(row.calendar_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(calendar) = calendar else {
        return Ok(());
    };
    let zone = settings::load(tx, calendar.member_id).await?.zone();
    let now = standing(&row, &calendar, zone, Utc::now());
    publish(state, tx, workspace, &row, &calendar, now).await
}

/// Announce, in `tx`, that every invitation of `calendars` announced as
/// awaiting an answer no longer does, `because`: for calendars about to be
/// deleted with their events (a removed connection, a member who left).
pub async fn close_all(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    calendars: &[CalendarRow],
    because: InvitationClosed,
) -> Result<()> {
    for calendar in calendars {
        let open = sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
            "SELECT {EVENT_COLUMNS} FROM schedule.events e
              WHERE e.calendar_id = $1 AND e.invitation_open
                FOR UPDATE"
        )))
        .bind(calendar.id)
        .fetch_all(&mut **tx)
        .await?;
        for row in open {
            publish(state, tx, workspace, &row, calendar, Err(because)).await?;
        }
    }
    Ok(())
}

/// Announce, in `tx`, every event of `calendar` that is or was an
/// invitation, as it stands now: after the calendar was turned on or off.
pub async fn announce_calendar(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    calendar: &CalendarRow,
) -> Result<()> {
    let rows = sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {EVENT_COLUMNS} FROM schedule.events e
          WHERE e.calendar_id = $1
            AND (e.invitation_open OR e.response = 'needs_action')
            FOR UPDATE"
    )))
    .bind(calendar.id)
    .fetch_all(&mut **tx)
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let zone = settings::load(tx, calendar.member_id).await?.zone();
    let now = Utc::now();
    for row in rows {
        let standing = standing(&row, calendar, zone, now);
        publish(state, tx, workspace, &row, calendar, standing).await?;
    }
    Ok(())
}

/// Publish `standing` for `row` with the next revision, unless it neither
/// awaits an answer now nor did before.
async fn publish(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    row: &EventRow,
    calendar: &CalendarRow,
    standing: std::result::Result<InvitationFacts, InvitationClosed>,
) -> Result<()> {
    let open = standing.is_ok();
    if !open && !row.invitation_open {
        return Ok(());
    }
    let revision: i64 = sqlx::query_scalar(
        "UPDATE schedule.events
            SET invitation_revision = invitation_revision + 1, invitation_open = $2
          WHERE id = $1
      RETURNING invitation_revision",
    )
    .bind(row.id)
    .bind(open)
    .fetch_one(&mut **tx)
    .await?;
    let (invitation, closed_because) = match standing {
        Ok(facts) => (Some(facts), None),
        Err(why) => (None, Some(why)),
    };
    state
        .events
        .publish(
            tx,
            &InvitationChanged {
                workspace: workspace.to_string(),
                event: row.id,
                calendar: calendar.id,
                owner: calendar.member_id,
                revision,
                invitation,
                closed_because,
            },
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_service_address_is_no_person_and_none_is_unknown() {
        assert_eq!(organizer_is_person(Some("joao@example.com")), Some(true));
        assert_eq!(
            organizer_is_person(Some("no-reply@webinars.example.com")),
            Some(false)
        );
        assert_eq!(organizer_is_person(Some("events@example.com")), Some(false));
        assert_eq!(organizer_is_person(Some("  ")), None);
        assert_eq!(organizer_is_person(None), None);
    }
}
