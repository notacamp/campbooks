//! `GET /api/v1/w/{workspace_id}/attention` — what needs the caller, in the
//! order Home shows it; or, with `state=cleared`, what left their list today
//! (Done today); or, with `state=waiting`, who the caller is waiting on
//! (campbooks-cloud#293).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        ItemRow, ItemView, OVERDUE_DAYS, Reason,
        bands::{self, BandInput, SortKey},
        visible_containers,
    },
    scopes::AttentionRead,
};

/// Open items, or what left the list today.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListState {
    /// What needs the caller now (the default).
    #[default]
    Open,
    /// What was cleared or put off since the start of the caller's day.
    Cleared,
    /// Who the caller is waiting on (`waiting` items), soonest nudge first.
    Waiting,
}

/// Which items.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
    /// `open` (default), `cleared` or `waiting`.
    #[serde(default)]
    #[param(inline)]
    pub state: ListState,
    /// The caller's offset from UTC, in minutes (-840 to 840; default 0,
    /// UTC): where their day starts, for Done today and for what is overdue.
    #[serde(default)]
    pub utc_offset: i32,
}

/// The widest offset from UTC a time zone has (UTC+14:00), in minutes.
const MAX_OFFSET_MINUTES: i32 = 14 * 60;

/// The caller's open items: in containers they may see (`$2`), not waits
/// (`state=waiting` lists them), not more than [`OVERDUE_DAYS`] past their
/// due date (`$3`) unless a bill or a nudge (each stays until a member
/// answers it), not lapsed (what they ask is no longer possible,
/// campbooks-cloud#275) and not snoozed by the caller (`$4`) at `$5`, with
/// the facts their band is worked out from. A task (campbooks-cloud#43)
/// stays however late, is left out while planned Later (`hidden_until`) and
/// replaces its conversation's `do` or `pay` item for its assignee (`$6`,
/// the caller as text) while it is open, and once closed for as long as that
/// item is the one it came from (cleared at or after the item's `since`):
/// finishing the task finishes the item. The order is worked out afterwards,
/// by band ([`bands`]).
const OPEN: &str = "SELECT i.id, i.record_type, i.record_id, i.container_type, i.container_id, \
            i.cause_type, i.cause_id, i.reason, i.trust, i.due_on, i.urgent, i.since, i.state, \
            i.cleared_because, i.cleared_by, i.cleared_at, NULL::timestamptz AS snoozed_until, \
            i.warning, i.from_person, i.plan, i.plan_on \
       FROM attention.items i \
      WHERE i.workspace_id = $1 AND i.state = 'open' AND i.reason <> 'waiting' \
        AND i.container_type || ':' || i.container_id = ANY($2) \
        AND (i.due_on IS NULL OR i.due_on >= $3 OR i.reason IN ('pay', 'nudge') \
             OR i.record_type = 'tasks.task') \
        AND (i.hidden_until IS NULL OR i.hidden_until <= $5) \
        AND NOT (i.record_type = 'conversations.conversation' AND i.reason IN ('do', 'pay') \
                 AND EXISTS (SELECT 1 FROM attention.items t \
                              WHERE t.workspace_id = i.workspace_id \
                                AND t.record_type = 'tasks.task' \
                                AND (t.state = 'open' OR t.cleared_at >= i.since) \
                                AND t.cause_type = i.record_type AND t.cause_id = i.record_id \
                                AND t.container_type = 'tasks.assignee' \
                                AND t.container_id = $6)) \
        AND (i.lapses_at IS NULL OR i.lapses_at > $5) \
        AND NOT EXISTS (SELECT 1 FROM attention.snoozes s \
                         WHERE s.item_id = i.id AND s.user_id = $4 AND s.until > $5)";

/// An open item with the facts its band is worked out from.
#[derive(sqlx::FromRow)]
struct OpenRow {
    #[sqlx(flatten)]
    item: ItemRow,
    /// Whether it is the warning about suspicious mail.
    warning: bool,
    /// Whether a person sent its cause; `None` when unknown.
    from_person: Option<bool>,
    /// A task's plan (`none`, `date`, `later`); `None` for a task projected
    /// before plans were kept, and for every other item.
    plan: Option<String>,
    /// A task's planned day.
    plan_on: Option<NaiveDate>,
}

/// Record type of a task.
const TASK: &str = "tasks.task";

/// Record type of an invitation's event (campbooks-cloud#360).
const INVITATION: &str = crate::features::project_invitation::EVENT;

/// What left the caller's list since `$5` (the start of their day): cleared,
/// but not replaced, cleared by a rule, opened ("Worth a look") or lapsed
/// (none of which a member marked), or snoozed by the caller (`$3`) and still snoozed at `$4`; newest
/// first.
const CLEARED: &str = "SELECT i.id, i.record_type, i.record_id, i.container_type, \
            i.container_id, i.cause_type, i.cause_id, i.reason, i.trust, i.due_on, i.urgent, \
            i.since, i.state, i.cleared_because, i.cleared_by, i.cleared_at, \
            s.until AS snoozed_until \
       FROM attention.items i \
       LEFT JOIN attention.snoozes s ON s.item_id = i.id AND s.user_id = $3 AND s.until > $4 \
      WHERE i.workspace_id = $1 \
        AND i.container_type || ':' || i.container_id = ANY($2) \
        AND i.reason IS NOT NULL \
        AND ((i.state = 'cleared' AND i.cleared_at >= $5 \
              AND coalesce(i.cleared_because, '') \
                  NOT IN ('superseded', 'rule', 'opened', 'lapsed')) \
             OR (i.state = 'open' AND i.reason <> 'waiting' AND s.created_at >= $5)) \
      ORDER BY greatest(i.cleared_at, s.created_at) DESC, i.id";

/// The caller's open waits (`waiting` items) in containers they may see
/// (`$2`), not snoozed by the caller (`$3`) at `$4`: soonest due date (when
/// Scout nudges) first, then oldest cause first.
const WAITING: &str = "SELECT i.id, i.record_type, i.record_id, i.container_type, \
            i.container_id, i.cause_type, i.cause_id, i.reason, i.trust, i.due_on, i.urgent, \
            i.since, i.state, i.cleared_because, i.cleared_by, i.cleared_at, \
            NULL::timestamptz AS snoozed_until \
       FROM attention.items i \
      WHERE i.workspace_id = $1 AND i.state = 'open' AND i.reason = 'waiting' \
        AND i.container_type || ':' || i.container_id = ANY($2) \
        AND NOT EXISTS (SELECT 1 FROM attention.snoozes s \
                         WHERE s.item_id = i.id AND s.user_id = $3 AND s.until > $4) \
      ORDER BY i.due_on ASC NULLS LAST, i.since ASC, i.id";

/// How many open items there are for one reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReasonCount {
    /// The reason.
    pub reason: Reason,
    /// Open items with it.
    pub count: u32,
}

/// The caller's items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AttentionList {
    /// Open: by band (now, today, week, whenever), then inside a band
    /// warnings first, dated before undated and the soonest date first,
    /// people before services, oldest first; each with its band and why.
    /// Cleared: newest first. Waiting: soonest nudge first.
    pub items: Vec<ItemView>,
    /// Open items per reason, every reason of the queue listed (all but
    /// `waiting`); for `state=waiting`, the `waiting` count alone; empty for
    /// cleared.
    pub counts: Vec<ReasonCount>,
    /// Open task items left off the list because they are not for today:
    /// not overdue, not due today or tomorrow, not planned for today or a
    /// day before, not back from Later (campbooks-cloud#358). 0 for cleared
    /// and waiting.
    pub tasks_elsewhere: u32,
}

/// List what needs you.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/attention",
    operation_id = "attention.list",
    tag = "attention",
    security(("bearer" = ["attention:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["attention.item"]).access("attention.item", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), ListQuery),
    responses(
        (status = OK, description = "The caller's items.", body = AttentionList),
        (status = BAD_REQUEST, description = "Not a state (`open`, `cleared`, `waiting`), or an offset beyond 14 hours (`invalid_offset`).", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<AttentionRead>,
    extract::Query(query): extract::Query<ListQuery>,
) -> Result<Json<AttentionList>> {
    if query.utc_offset.abs() > MAX_OFFSET_MINUTES {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_offset")));
    }
    let access = &workspace.access;
    campbooks_kernel::authz::require_type(
        &state,
        access,
        campbooks_kernel::authz::Action::Read,
        crate::domain::resources::ITEM_RECORD,
        || Error::Forbidden,
    )?;
    let now = Utc::now();
    let offset = TimeDelta::minutes(i64::from(query.utc_offset));
    // The caller's own today.
    let today = (now + offset).date_naive();
    let mut tx = state.db.begin_for(access.workspace).await?;
    let visible = visible_containers(&state, &mut tx, access).await?;
    let user = access.user.map(|u| *u.as_uuid());
    // Runtime queries, so the columns added since (`lapses_at`, the band's
    // facts) need no entry in the `.sqlx` offline cache.
    let (items, tasks_elsewhere) = match query.state {
        ListState::Open => {
            let rows: Vec<OpenRow> = sqlx::query_as(OPEN)
                .bind(access.workspace.as_uuid())
                .bind(visible.as_slice())
                .bind(today - TimeDelta::days(OVERDUE_DAYS))
                .bind(user)
                .bind(now)
                .bind(access.user.map(|u| u.to_string()))
                .fetch_all(&mut *tx)
                .await?;
            tx.commit().await?;
            ranked(rows, today, now)?
        }
        ListState::Cleared => {
            // Midnight where the caller is, as an instant.
            let start = today.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc() - offset;
            let rows: Vec<ItemRow> = sqlx::query_as(CLEARED)
                .bind(access.workspace.as_uuid())
                .bind(visible.as_slice())
                .bind(user)
                .bind(now)
                .bind(start)
                .fetch_all(&mut *tx)
                .await?;
            tx.commit().await?;
            (views(rows, today)?, 0)
        }
        ListState::Waiting => {
            let rows: Vec<ItemRow> = sqlx::query_as(WAITING)
                .bind(access.workspace.as_uuid())
                .bind(visible.as_slice())
                .bind(user)
                .bind(now)
                .fetch_all(&mut *tx)
                .await?;
            tx.commit().await?;
            (views(rows, today)?, 0)
        }
    };
    let count = |reason: Reason| ReasonCount {
        reason,
        count: u32::try_from(items.iter().filter(|i| i.reason == reason).count())
            .unwrap_or(u32::MAX),
    };
    let counts = match query.state {
        ListState::Open => Reason::QUEUE.into_iter().map(count).collect(),
        ListState::Waiting => vec![count(Reason::Waiting)],
        ListState::Cleared => Vec::new(),
    };
    Ok(Json(AttentionList {
        items,
        counts,
        tasks_elsewhere,
    }))
}

/// The views of `rows`, as listed.
fn views(rows: Vec<ItemRow>, today: NaiveDate) -> Result<Vec<ItemView>> {
    rows.into_iter().map(|row| row.view(today)).collect()
}

/// Whether a task is for today: overdue, due today or tomorrow, planned for
/// today or a day before, or back from Later (the list only reads a Later
/// task once its time has passed). A task projected before plans were kept
/// (`plan` unknown) stays as it was shown until it next changes.
fn for_today(row: &OpenRow, today: NaiveDate) -> bool {
    let Some(plan) = row.plan.as_deref() else {
        return true;
    };
    let due = row
        .item
        .due_on
        .is_some_and(|due| due <= today + TimeDelta::days(1));
    let planned = match plan {
        "date" => row.plan_on.is_some_and(|day| day <= today),
        "later" => true,
        _ => false,
    };
    due || planned
}

/// The open list on the caller's `today` at `now`: each item with its band
/// and why, in Home's order (by band, then [`bands::sort_key`]), and how
/// many task items were left out because they are not for today.
fn ranked(
    rows: Vec<OpenRow>,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<(Vec<ItemView>, u32)> {
    let mut elsewhere = 0_u32;
    let mut keyed: Vec<(SortKey, ItemView)> = Vec::with_capacity(rows.len());
    for row in rows {
        if row.item.record_type == TASK && !for_today(&row, today) {
            elsewhere = elsewhere.saturating_add(1);
            continue;
        }
        let planned_on = row.plan_on.filter(|_| row.plan.as_deref() == Some("date"));
        let event = row.item.record_type == INVITATION;
        let mut view = row.item.view(today)?;
        let input = BandInput {
            id: view.id,
            reason: view.reason,
            warning: row.warning,
            from_person: row.from_person,
            due_on: view.due_on,
            planned_on,
            since: view.since,
            event,
        };
        let (band, reason) = bands::band(&input, today, now);
        view.band = Some(band);
        view.band_reason = Some(reason);
        keyed.push((bands::sort_key(&input, band), view));
    }
    keyed.sort_by_key(|(key, _)| *key);
    Ok((keyed.into_iter().map(|(_, view)| view).collect(), elsewhere))
}

#[cfg(test)]
mod tests;
