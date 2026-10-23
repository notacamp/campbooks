//! `schedule.add_read_items` ← `conversations.message_read`: the things that
//! happen at a time, which Scout read in an email, become events on the
//! Campbooks calendar of the inbox's personal owner.
//!
//! - Keyed by the item's key: a repeated delivery changes nothing, and a
//!   later email's item that `updates` or `cancels` an earlier one changes or
//!   cancels that item's event.
//! - Nothing is added for a shared inbox (whose calendar would it be?), for a
//!   kind the member turned off, or for an item whose event the member
//!   removed (it is remembered).
//! - An item from a sender's own invitation whose calendar `UID` an event of
//!   the member already has links that event to the email instead.
//! - Where the member set an "also add to" calendar for the kind, the event
//!   is then added there as by "Add to my calendar".
//!
//! The titles and places are not in the event: they are in the blob it
//! points at, which this module reads and the purges delete.

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError, RecordRef,
    tenancy::{UserId, WorkspaceId},
};
use sqlx::{PgConnection, Postgres, Transaction, types::Uuid};

use crate::{
    domain::{
        blobs::read_json,
        moment::{Moment, Span, parse_zone},
        publish,
        rows::{
            CalendarRow, EVENT_COLUMNS, EventRow, calendar_by_id, ensure_campbooks_calendar,
            event_by_id,
        },
        settings::{self, Settings},
        write::{MAX_PLACE, MAX_TITLE, NewRow, insert, tidy},
    },
    features::push_event::push_row,
    inbound::{MessageRead, ReadItem, ReadItemText, ReadTexts},
    model::{EventKind, EventOrigin},
};

/// Largest `texts` document read.
const MAX_TEXTS_BYTES: usize = 1024 * 1024;

/// Makes events of what Scout read.
pub struct AddReadItems;

impl EventHandler for AddReadItems {
    type Event = MessageRead;
    const NAME: &'static str = "schedule.add_read_items";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<MessageRead>) -> Result<(), JobError> {
        let read = event.payload;
        // A shared inbox adds nothing; neither does a read with no item.
        let Some(owner) = read.owner else {
            return Ok(());
        };
        if read.items.is_empty() {
            return Ok(());
        }
        let workspace: WorkspaceId = read
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("message_read names an invalid workspace"))?;
        let state = ctx.state().clone();
        let texts: ReadTexts = match &read.texts {
            Some(blob) => read_json(&state.blobs, &read.workspace, blob, MAX_TEXTS_BYTES).await?,
            None => ReadTexts::default(),
        };
        let tx = ctx.tx().await?;
        let settings = settings::load(tx, owner).await?;
        let calendar = ensure_campbooks_calendar(tx, workspace, owner).await?;
        let found_by = if read.layer == "structured" {
            "structured"
        } else {
            "model"
        };
        let source = RecordRef::new("conversations.message", &read.message);
        let place = Place {
            state: &state,
            workspace,
            owner,
            settings: &settings,
            calendar: &calendar,
            found_by,
            source: &source,
            conversation: read.conversation.parse::<Uuid>().ok(),
        };
        for item in &read.items {
            let text = texts.items.iter().find(|text| text.key == item.key);
            add_item(tx, &place, item, text).await?;
        }
        Ok(())
    }
}

/// Where a read's items go.
struct Place<'a> {
    state: &'a AppState,
    workspace: WorkspaceId,
    owner: UserId,
    settings: &'a Settings,
    calendar: &'a CalendarRow,
    found_by: &'a str,
    source: &'a RecordRef,
    conversation: Option<Uuid>,
}

/// The member's event any of whose item keys is `key`.
async fn event_of_key(
    conn: &mut PgConnection,
    owner: UserId,
    key: &str,
) -> Result<Option<EventRow>, sqlx::Error> {
    sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {EVENT_COLUMNS} FROM schedule.events e
           JOIN schedule.calendars c ON c.id = e.calendar_id
          WHERE c.member_id = $1 AND $2 = ANY(e.item_keys)
            FOR UPDATE OF e"
    )))
    .bind(owner)
    .bind(key)
    .fetch_optional(conn)
    .await
}

#[allow(clippy::too_many_lines)]
async fn add_item(
    tx: &mut Transaction<'static, Postgres>,
    place: &Place<'_>,
    item: &ReadItem,
    text: Option<&ReadItemText>,
) -> Result<(), JobError> {
    let (state, workspace, owner) = (place.state, place.workspace, place.owner);
    // A delivery repeated, or an item already applied.
    if event_of_key(tx, owner, &item.key).await?.is_some() {
        return Ok(());
    }
    let removed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM schedule.removed_items
                         WHERE member_id = $1 AND item_key = $2)",
    )
    .bind(owner)
    .bind(&item.key)
    .fetch_one(&mut **tx)
    .await?;
    if removed {
        return Ok(());
    }
    let Ok(kind) = item.kind.parse::<EventKind>() else {
        tracing::info!(reason = "kind", "an item of a read is not added");
        return Ok(());
    };
    if !place.settings.scout_kinds.contains(&kind) {
        return Ok(());
    }

    // A cancellation changes what was added, and adds nothing.
    if let Some(cancels) = &item.cancels {
        if let Some(row) = event_of_key(tx, owner, cancels).await?
            && row.deleted_at.is_none()
            && row.status != "cancelled"
        {
            sqlx::query(
                "UPDATE schedule.events
                    SET status = 'cancelled', item_keys = array_append(item_keys, $2),
                        updated_at = now()
                  WHERE id = $1",
            )
            .bind(row.id)
            .bind(&item.key)
            .execute(&mut **tx)
            .await?;
            if let Some(now) = event_by_id(tx, row.id).await? {
                publish::cancelled(state, tx, workspace, owner, &now).await?;
            }
        }
        return Ok(());
    }

    let Some(start) = Moment::parse(&item.starts) else {
        tracing::info!(reason = "time", "an item of a read is not added");
        return Ok(());
    };
    let end = item.ends.as_deref().and_then(Moment::parse);
    let Some(span) = Span::new(start, end).or_else(|| Span::new(start, None)) else {
        tracing::info!(reason = "time", "an item of a read is not added");
        return Ok(());
    };
    let time_zone = item
        .time_zone
        .as_deref()
        .filter(|zone| parse_zone(zone).is_some())
        .filter(|_| !span.start.is_day());
    let title = text
        .map(|text| text.title.trim())
        .filter(|title| title.chars().count() <= MAX_TITLE)
        .unwrap_or_default();
    let spot = tidy(text.and_then(|text| text.place.as_deref()))
        .filter(|spot| spot.chars().count() <= MAX_PLACE);

    // A later email changes an event added before.
    if let Some(updates) = &item.updates
        && let Some(row) = event_of_key(tx, owner, updates).await?
    {
        if row.deleted_at.is_some() {
            return Ok(());
        }
        let (starts_at, ends_at, starts_on, ends_on) = span.columns();
        sqlx::query(
            "UPDATE schedule.events
                SET kind = $2, title = coalesce(nullif($3, ''), title), place = coalesce($4, place),
                    starts_at = $5, ends_at = $6, starts_on = $7, ends_on = $8, all_day = $9,
                    time_zone = coalesce($10, time_zone), status = 'confirmed',
                    item_keys = array_append(item_keys, $11), updated_at = now()
              WHERE id = $1",
        )
        .bind(row.id)
        .bind(kind.as_str())
        .bind(title)
        .bind(&spot)
        .bind(starts_at)
        .bind(ends_at)
        .bind(starts_on)
        .bind(ends_on)
        .bind(span.start.is_day())
        .bind(time_zone)
        .bind(&item.key)
        .execute(&mut **tx)
        .await?;
        if let Some(now) = event_by_id(tx, row.id).await? {
            publish::changed(state, tx, workspace, owner, &now).await?;
        }
        return Ok(());
    }

    // The sender's own invitation is already on one of the member's calendars.
    if let Some(uid) = text.and_then(|text| text.ical_uid.as_deref()) {
        let linked = sqlx::query_as::<_, EventRow>(sqlx::AssertSqlSafe(format!(
            "SELECT {EVENT_COLUMNS} FROM schedule.events e
               JOIN schedule.calendars c ON c.id = e.calendar_id
              WHERE c.member_id = $1 AND (c.source = 'campbooks' OR c.syncing)
                AND e.ical_uid = $2 AND e.deleted_at IS NULL
              LIMIT 1
                FOR UPDATE OF e"
        )))
        .bind(owner)
        .bind(uid)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(row) = linked {
            sqlx::query(
                "UPDATE schedule.events
                    SET source_type = coalesce(source_type, $2),
                        source_id = coalesce(source_id, $3),
                        item_keys = array_append(item_keys, $4), updated_at = now()
                  WHERE id = $1",
            )
            .bind(row.id)
            .bind(&place.source.record_type)
            .bind(&place.source.id)
            .bind(&item.key)
            .execute(&mut **tx)
            .await?;
            return Ok(());
        }
    }

    let id = crate::model::EventId::new();
    insert(
        tx,
        &NewRow {
            id,
            workspace,
            calendar: place.calendar.id,
            kind,
            title,
            span,
            time_zone,
            place: spot.as_deref(),
            notes: None,
            origin: EventOrigin::Scout,
            found_by: Some(place.found_by),
            source: Some(place.source),
            item_key: Some(&item.key),
            ical_uid: text.and_then(|text| text.ical_uid.as_deref()),
            busy: true,
            pushed_from: None,
            series_id: None,
            original_start: None,
            source_conversation: place.conversation,
        },
    )
    .await?;
    let Some(row) = event_by_id(tx, id).await? else {
        return Ok(());
    };
    publish::created(state, tx, workspace, owner, &row).await?;

    // Where the member wants this kind of event also added.
    if let Some(target) = place.settings.also_add_to.get(kind.as_str()) {
        let target = calendar_by_id(tx, *target).await;
        if let Ok(target) = target
            && target.member_id == owner
            && !target.is_campbooks()
            && target.writable
            && target.syncing
        {
            push_row(state, tx, workspace, owner, &row, &target).await?;
        }
    }
    Ok(())
}
