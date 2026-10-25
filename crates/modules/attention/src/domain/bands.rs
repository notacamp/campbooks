//! Priority bands (campbooks-cloud#358): where an open item stands on Home,
//! whatever its reason.
//!
//! Every open item falls in one band, Now, Today, This week or Whenever, by
//! the first rule its facts meet ([`band`]), and the rule is kept as the
//! reason Home shows ("Overdue · 2 days", "Marta has waited 4 days").
//! Inside a band the order is fixed ([`sort_key`]): warnings first, then
//! dated before undated, soonest date first, then people before services,
//! then the oldest cause first. Facts only: no model is asked, and nothing
//! here reads what an email says.

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{ItemId, Reason};

/// How many whole days someone has waited for an answer before it is for
/// today.
pub const WAITED_DAYS: i64 = 3;

/// How many days ahead a due date makes an item for this week.
pub const WEEK_DAYS: i64 = 7;

/// Where an item stands on Home, in the order Home shows the bands.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    /// Scam warnings and anything overdue.
    Now,
    /// Due today or tomorrow, planned for today, nudges that are due, and
    /// people who have waited 3 days or more.
    Today,
    /// Dated in the next 7 days, and people waiting for an answer.
    Week,
    /// No date, from services, or kept on Home by a rule the member taught.
    Whenever,
}

/// The rule that put an item in its band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BandRule {
    /// It is the warning about suspicious mail (Now).
    Warning,
    /// Its due date has passed (Now); `days` late.
    Overdue,
    /// It is due today (Today).
    DueToday,
    /// It is due tomorrow (Today).
    DueTomorrow,
    /// A task the member planned for today or a day before (Today).
    PlannedToday,
    /// Someone the member waits on has not written by when they were
    /// expected to (Today); `on` is that day.
    NudgeDue,
    /// A person has waited `days` whole days for an answer, 3 or more
    /// (Today).
    Waited,
    /// Its due date, `on`: within the next 7 days (This week), or later
    /// (Whenever).
    DueOn,
    /// A person is waiting for an answer or a choice (This week).
    WaitingAnswer,
    /// A rule the member taught keeps it on Home (Whenever).
    KeptByRule,
    /// Nothing dates it (Whenever).
    NoDate,
}

/// Why an item is in its band: the rule, with the number of days or the
/// day it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BandReason {
    /// The rule.
    pub rule: BandRule,
    /// Days late (`overdue`) or waited (`waited`).
    pub days: Option<u32>,
    /// The day it names: the due date (`due_on`), or when an answer was
    /// expected (`nudge_due`).
    pub on: Option<NaiveDate>,
}

impl BandReason {
    const fn of(rule: BandRule) -> Self {
        Self {
            rule,
            days: None,
            on: None,
        }
    }

    fn days(rule: BandRule, days: i64) -> Self {
        Self {
            days: Some(u32::try_from(days.max(0)).unwrap_or(u32::MAX)),
            ..Self::of(rule)
        }
    }

    const fn on(rule: BandRule, on: Option<NaiveDate>) -> Self {
        Self {
            on,
            ..Self::of(rule)
        }
    }
}

/// The facts of an open item that decide its band and its place in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandInput {
    /// The item: the last tie-break.
    pub id: ItemId,
    /// Why it needs the member.
    pub reason: Reason,
    /// Whether it is the warning about suspicious mail.
    pub warning: bool,
    /// Whether a person sent its cause rather than a machine; `None` when
    /// unknown.
    pub from_person: Option<bool>,
    /// When it is due. For a nudge, when an answer was expected.
    pub due_on: Option<NaiveDate>,
    /// The day a task is planned for, when its plan is a day.
    pub planned_on: Option<NaiveDate>,
    /// When its cause arrived.
    pub since: DateTime<Utc>,
}

impl BandInput {
    /// The day that dates it inside its band: its due date, else the day it
    /// is planned for.
    fn date(&self) -> Option<NaiveDate> {
        self.due_on.or(self.planned_on)
    }

    /// Whether a person sent it.
    fn sent_by_person(&self) -> bool {
        self.from_person == Some(true)
    }
}

/// The band of `item` and why, on the member's `today` at `now`: the first
/// rule it meets.
///
/// 1. A warning: Now.
/// 2. Past its due date: Now, overdue.
/// 3. Due today or tomorrow: Today.
/// 4. A task planned for today or a day before: Today.
/// 5. A nudge: Today. A nudge's date is when an answer was expected, so it
///    is never overdue or due by it.
/// 6. A reply a person has waited 3 days or more for: Today.
/// 7. Due within the next 7 days: This week.
/// 8. A reply or a choice a person is waiting for: This week.
/// 9. Kept on Home by the member's rule ("Worth a look"): Whenever.
/// 10. Anything else: Whenever, by its due date when it has one, else
///     undated.
pub fn band(item: &BandInput, today: NaiveDate, now: DateTime<Utc>) -> (Band, BandReason) {
    let nudge = item.reason == Reason::Nudge;
    let due = item.due_on.filter(|_| !nudge);
    if item.warning {
        return (Band::Now, BandReason::of(BandRule::Warning));
    }
    if let Some(due) = due {
        if due < today {
            let late = (today - due).num_days();
            return (Band::Now, BandReason::days(BandRule::Overdue, late));
        }
        if due == today {
            return (Band::Today, BandReason::of(BandRule::DueToday));
        }
        if due == today + TimeDelta::days(1) {
            return (Band::Today, BandReason::of(BandRule::DueTomorrow));
        }
    }
    if item.planned_on.is_some_and(|day| day <= today) {
        return (Band::Today, BandReason::of(BandRule::PlannedToday));
    }
    if nudge {
        return (Band::Today, BandReason::on(BandRule::NudgeDue, item.due_on));
    }
    let waited = (now - item.since).num_days();
    if item.reason == Reason::Reply && item.sent_by_person() && waited >= WAITED_DAYS {
        return (Band::Today, BandReason::days(BandRule::Waited, waited));
    }
    if let Some(due) = due
        && due <= today + TimeDelta::days(WEEK_DAYS)
    {
        return (Band::Week, BandReason::on(BandRule::DueOn, Some(due)));
    }
    if matches!(item.reason, Reason::Reply | Reason::Decide) && item.sent_by_person() {
        return (Band::Week, BandReason::of(BandRule::WaitingAnswer));
    }
    if item.reason == Reason::WorthALook {
        return (Band::Whenever, BandReason::of(BandRule::KeptByRule));
    }
    match due {
        Some(due) => (Band::Whenever, BandReason::on(BandRule::DueOn, Some(due))),
        None => (Band::Whenever, BandReason::of(BandRule::NoDate)),
    }
}

/// Where an item goes on Home: by band, then warnings first, dated before
/// undated, the soonest date first (the longest overdue first), people
/// before services (and the unknown), the oldest cause first, then the item
/// id, so the order never changes between two reads.
pub type SortKey = (
    Band,
    bool,
    bool,
    Option<NaiveDate>,
    bool,
    DateTime<Utc>,
    ItemId,
);

/// The key that orders `item`, in `band`, on Home.
pub fn sort_key(item: &BandInput, band: Band) -> SortKey {
    let date = item.date();
    (
        band,
        !item.warning,
        date.is_none(),
        date,
        !item.sent_by_person(),
        item.since,
        item.id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 27).unwrap()
    }

    fn now() -> DateTime<Utc> {
        today().and_hms_opt(12, 0, 0).unwrap().and_utc()
    }

    fn day(offset: i64) -> NaiveDate {
        today() + TimeDelta::days(offset)
    }

    fn item(reason: Reason) -> BandInput {
        BandInput {
            id: ItemId::new(),
            reason,
            warning: false,
            from_person: None,
            due_on: None,
            planned_on: None,
            since: now() - TimeDelta::hours(1),
        }
    }

    fn of(item: &BandInput) -> (Band, BandReason) {
        band(item, today(), now())
    }

    #[test]
    fn a_warning_is_now_whatever_else_it_carries() {
        let mut warning = item(Reason::Review);
        warning.warning = true;
        warning.due_on = Some(day(5));
        warning.from_person = Some(true);
        assert_eq!(of(&warning), (Band::Now, BandReason::of(BandRule::Warning)));
    }

    #[test]
    fn past_its_due_date_is_now_with_the_days_late() {
        for reason in [Reason::Pay, Reason::Do, Reason::Reply, Reason::Review] {
            let mut late = item(reason);
            late.due_on = Some(day(-2));
            assert_eq!(
                of(&late),
                (Band::Now, BandReason::days(BandRule::Overdue, 2)),
                "{reason:?}"
            );
        }
        let mut yesterday = item(Reason::Pay);
        yesterday.due_on = Some(day(-1));
        assert_eq!(of(&yesterday).1.days, Some(1));
    }

    #[test]
    fn due_today_or_tomorrow_is_today() {
        let mut due = item(Reason::Do);
        due.due_on = Some(today());
        assert_eq!(of(&due), (Band::Today, BandReason::of(BandRule::DueToday)));
        due.due_on = Some(day(1));
        assert_eq!(
            of(&due),
            (Band::Today, BandReason::of(BandRule::DueTomorrow))
        );
    }

    #[test]
    fn a_task_planned_for_today_or_before_is_today() {
        for planned in [today(), day(-3)] {
            let mut task = item(Reason::Do);
            task.planned_on = Some(planned);
            assert_eq!(
                of(&task),
                (Band::Today, BandReason::of(BandRule::PlannedToday)),
                "{planned}"
            );
        }
        // Planned for later in the week, due in two days: by its due date.
        let mut later = item(Reason::Do);
        later.planned_on = Some(day(3));
        later.due_on = Some(day(2));
        assert_eq!(
            of(&later),
            (Band::Week, BandReason::on(BandRule::DueOn, Some(day(2))))
        );
    }

    #[test]
    fn a_nudge_is_today_and_never_overdue_by_its_expected_day() {
        let mut nudge = item(Reason::Nudge);
        nudge.due_on = Some(day(-10));
        assert_eq!(
            of(&nudge),
            (
                Band::Today,
                BandReason::on(BandRule::NudgeDue, Some(day(-10)))
            )
        );
        nudge.due_on = None;
        assert_eq!(
            of(&nudge),
            (Band::Today, BandReason::on(BandRule::NudgeDue, None))
        );
    }

    #[test]
    fn a_person_who_waited_three_days_is_today() {
        let mut reply = item(Reason::Reply);
        reply.from_person = Some(true);
        // Exactly three days: today.
        reply.since = now() - TimeDelta::days(3);
        assert_eq!(
            of(&reply),
            (Band::Today, BandReason::days(BandRule::Waited, 3))
        );
        // A minute short of it: this week, waiting for an answer.
        reply.since = now() - TimeDelta::days(3) + TimeDelta::minutes(1);
        assert_eq!(
            of(&reply),
            (Band::Week, BandReason::of(BandRule::WaitingAnswer))
        );
        // Four and a half days: four whole days.
        reply.since = now() - TimeDelta::hours(108);
        assert_eq!(of(&reply).1.days, Some(4));
    }

    #[test]
    fn a_service_or_an_unknown_sender_never_waits() {
        for from_person in [Some(false), None] {
            let mut reply = item(Reason::Reply);
            reply.from_person = from_person;
            reply.since = now() - TimeDelta::days(9);
            assert_eq!(
                of(&reply),
                (Band::Whenever, BandReason::of(BandRule::NoDate)),
                "{from_person:?}"
            );
        }
        // Only a reply is waited for that long: a choice waits this week.
        let mut decide = item(Reason::Decide);
        decide.from_person = Some(true);
        decide.since = now() - TimeDelta::days(9);
        assert_eq!(
            of(&decide),
            (Band::Week, BandReason::of(BandRule::WaitingAnswer))
        );
    }

    #[test]
    fn due_within_the_week_is_this_week_and_later_is_whenever() {
        let mut bill = item(Reason::Pay);
        bill.due_on = Some(day(7));
        assert_eq!(
            of(&bill),
            (Band::Week, BandReason::on(BandRule::DueOn, Some(day(7))))
        );
        bill.due_on = Some(day(8));
        assert_eq!(
            of(&bill),
            (
                Band::Whenever,
                BandReason::on(BandRule::DueOn, Some(day(8)))
            )
        );
    }

    #[test]
    fn worth_a_look_is_kept_by_the_rule_and_the_rest_is_undated() {
        assert_eq!(
            of(&item(Reason::WorthALook)),
            (Band::Whenever, BandReason::of(BandRule::KeptByRule))
        );
        for reason in [Reason::Do, Reason::Pay, Reason::Review, Reason::Reply] {
            assert_eq!(
                of(&item(reason)),
                (Band::Whenever, BandReason::of(BandRule::NoDate)),
                "{reason:?}"
            );
        }
    }

    #[test]
    fn the_wire_names_are_the_contract() {
        assert_eq!(
            serde_json::to_value([Band::Now, Band::Today, Band::Week, Band::Whenever]).unwrap(),
            serde_json::json!(["now", "today", "week", "whenever"])
        );
        assert_eq!(
            serde_json::to_value(BandReason::days(BandRule::Overdue, 2)).unwrap(),
            serde_json::json!({"rule": "overdue", "days": 2, "on": null})
        );
        assert_eq!(
            serde_json::to_value(BandRule::WaitingAnswer).unwrap(),
            "waiting_answer"
        );
    }

    /// The order of `items` on Home, as the indexes they had.
    fn ranked(items: &[BandInput]) -> Vec<usize> {
        let mut keyed: Vec<(SortKey, usize)> = items
            .iter()
            .enumerate()
            .map(|(index, item)| (sort_key(item, of(item).0), index))
            .collect();
        keyed.sort();
        keyed.into_iter().map(|(_, index)| index).collect()
    }

    #[test]
    fn bands_come_in_order_whatever_the_reason() {
        let mut warning = item(Reason::Review);
        warning.warning = true;
        let mut overdue = item(Reason::Pay);
        overdue.due_on = Some(day(-1));
        let mut tomorrow = item(Reason::Reply);
        tomorrow.due_on = Some(day(1));
        let mut week = item(Reason::Do);
        week.due_on = Some(day(4));
        let look = item(Reason::WorthALook);
        // Given in reverse.
        let items = [look, week, tomorrow, overdue, warning];
        assert_eq!(ranked(&items), [4, 3, 2, 1, 0]);
    }

    #[test]
    fn inside_a_band_warnings_dates_people_then_age() {
        // Now: the warning first, then the longest overdue.
        let mut warning = item(Reason::Review);
        warning.warning = true;
        let mut late_two = item(Reason::Pay);
        late_two.due_on = Some(day(-2));
        let mut late_five = item(Reason::Do);
        late_five.due_on = Some(day(-5));
        assert_eq!(ranked(&[late_two, late_five, warning]), [2, 1, 0]);

        // This week: dated first, soonest first; then people waiting, the
        // oldest first.
        let mut friday = item(Reason::Pay);
        friday.due_on = Some(day(4));
        let mut wednesday = item(Reason::Do);
        wednesday.due_on = Some(day(2));
        let mut fresh = item(Reason::Reply);
        fresh.from_person = Some(true);
        fresh.since = now() - TimeDelta::hours(2);
        let mut older = item(Reason::Decide);
        older.from_person = Some(true);
        older.since = now() - TimeDelta::days(2);
        assert_eq!(ranked(&[fresh, older, friday, wednesday]), [3, 2, 1, 0]);

        // Whenever: people before services and the unknown, then the oldest.
        let mut person = item(Reason::Do);
        person.from_person = Some(true);
        person.since = now() - TimeDelta::hours(1);
        let mut service = item(Reason::Do);
        service.from_person = Some(false);
        service.since = now() - TimeDelta::days(5);
        let mut unknown = item(Reason::Do);
        unknown.since = now() - TimeDelta::days(6);
        assert_eq!(ranked(&[service, unknown, person]), [2, 1, 0]);
    }

    #[test]
    fn a_planned_day_dates_a_task_and_ties_go_by_id() {
        let mut planned = item(Reason::Do);
        planned.planned_on = Some(day(-1));
        let mut due = item(Reason::Do);
        due.due_on = Some(today());
        // Both Today: the planned day (yesterday) comes before today.
        assert_eq!(ranked(&[due, planned]), [1, 0]);

        let first = item(Reason::Do);
        let mut second = first.clone();
        second.id = ItemId::new();
        assert_eq!(ranked(&[second, first]), [1, 0]);
    }
}
