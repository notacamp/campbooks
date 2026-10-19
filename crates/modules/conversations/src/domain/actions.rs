//! Each need's own actions, from the catalogue (campbooks-cloud#166).
//!
//! The model pass rates every action of
//! [`campbooks_ai::decide::signals::actions`] for a message and [`store`]s
//! the likelihoods, only while `CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED`
//! is on. [`attach`] then gives each open need, in `conversations.list` and
//! `conversations.get`, the actions [`rank`] picks for it under the floor
//! configured now (`CAMPBOOKS_CONVERSATIONS__ACTIONS__FLOOR_PERCENT`), and
//! only while the flag is on: off, nothing is rated or read, so every need
//! reads exactly as before, whatever was rated while it was on (the flag is
//! a kill switch).
//!
//! [`rank`]: a need's own answers first (Report / It's safe on a warning,
//! Already paid / Still to pay on a bill: the question it asks, always
//! offered, likelier first), then the other actions its kind allows that
//! cleared the floor, likelier first, a pair of [`COMBOS`] as one in first
//! place; [`MAX_ACTIONS`] in all.

use campbooks_ai::decide::signals::actions::{ActionId, COMBOS};
use campbooks_kernel::Result;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, types::Uuid};
use utoipa::ToSchema;

use super::conversation::ConversationSummary;
use crate::{
    config::ActionSettings,
    model::{MessageId, NeedAction, NeedQuestion, NeedReason},
};

/// The most actions a need offers.
pub const MAX_ACTIONS: usize = 3;

/// One action a need offers: one of the catalogue's, or a pair usually done
/// together, offered as one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SuggestedAction {
    /// The action, or the pair's two in the order they are done.
    pub actions: Vec<NeedAction>,
    /// How likely the member is to want it, in percent (0 to 100), as the
    /// `decide` model rated it (a pair: the less likely of the two).
    pub likelihood: u32,
}

/// One action's rating, as stored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rated {
    /// The action.
    pub action: ActionId,
    /// How likely the member is to want it.
    pub likelihood: f64,
}

/// Store `rated` for `message` in `workspace`, replacing what was stored.
pub async fn store(
    conn: &mut PgConnection,
    (workspace, message): (Uuid, MessageId),
    rated: &[(ActionId, f64)],
    model: &str,
) -> Result<()> {
    sqlx::query!(
        "DELETE FROM conversations.action_likelihoods WHERE message_id = $1",
        message as MessageId,
    )
    .execute(&mut *conn)
    .await?;
    let actions: Vec<String> = rated.iter().map(|(a, _)| a.as_str().to_owned()).collect();
    let likelihoods: Vec<f64> = rated.iter().map(|(_, l)| *l).collect();
    sqlx::query!(
        r#"INSERT INTO conversations.action_likelihoods
               (message_id, workspace_id, action, likelihood, model)
           SELECT $1::uuid, $2::uuid, a.action, a.likelihood, $5::text
             FROM unnest($3::text[], $4::float8[]) AS a(action, likelihood)"#,
        message as MessageId,
        workspace,
        &actions,
        &likelihoods,
        model,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// With the catalogue on, give each summary's open need the actions
/// [`rank`] picks for it from what was stored about its message, under the
/// floor `settings` configure now; nothing when it is off, or nothing was
/// rated.
pub async fn attach(
    conn: &mut PgConnection,
    settings: ActionSettings,
    summaries: &mut [ConversationSummary],
) -> Result<()> {
    if !settings.enabled {
        return Ok(());
    }
    let messages: Vec<MessageId> = summaries
        .iter()
        .filter_map(|s| s.need.as_ref().map(|n| n.message_id))
        .collect();
    if messages.is_empty() {
        return Ok(());
    }
    let rows = sqlx::query!(
        r#"SELECT message_id AS "message_id: MessageId", action, likelihood
             FROM conversations.action_likelihoods
            WHERE message_id = ANY($1)"#,
        &messages as &[MessageId],
    )
    .fetch_all(conn)
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    for summary in summaries {
        let Some(need) = summary.need.as_mut() else {
            continue;
        };
        let rated: Vec<Rated> = rows
            .iter()
            .filter(|r| r.message_id == need.message_id)
            .filter_map(|r| {
                Some(Rated {
                    action: ActionId::parse(&r.action)?,
                    likelihood: r.likelihood,
                })
            })
            .collect();
        let ranked = rank(need.reason, need.question, &rated, settings.floor());
        need.actions = (!ranked.is_empty()).then_some(ranked);
    }
    Ok(())
}

/// The actions a need of `reason`, raised by `question`, offers from
/// `rated` under `floor` (see the module's documentation).
pub fn rank(
    reason: NeedReason,
    question: NeedQuestion,
    rated: &[Rated],
    floor: f64,
) -> Vec<SuggestedAction> {
    let (pair, allowed): (Option<[ActionId; 2]>, &[ActionId]) = match (reason, question) {
        // A wait offers the client's own actions (Nudge, Heard from them,
        // Later, Not waiting): nothing of the catalogue (campbooks-cloud#293).
        (NeedReason::Waiting | NeedReason::Nudge, _) => return Vec::new(),
        (NeedReason::Review, NeedQuestion::Suspicious) => {
            (Some([ActionId::Report, ActionId::Safe]), &[])
        }
        (NeedReason::Pay, _) => (
            Some([ActionId::AlreadyPaid, ActionId::StillToPay]),
            &[
                ActionId::OpenAttachment,
                ActionId::Remind,
                ActionId::Forward,
            ],
        ),
        _ => (
            None,
            &[
                ActionId::Reply,
                ActionId::OpenAttachment,
                ActionId::Remind,
                ActionId::Forward,
            ],
        ),
    };
    let find = |action: ActionId| rated.iter().find(|r| r.action == action);
    let likelier = |a: &Rated, b: &Rated| {
        b.likelihood
            .total_cmp(&a.likelihood)
            .then(a.action.cmp(&b.action))
    };
    let mut out = Vec::new();
    // The need's own question: both answers, always, likelier first.
    if let Some([first, second]) = pair
        && let (Some(first), Some(second)) = (find(first), find(second))
    {
        let mut both = [*first, *second];
        both.sort_by(likelier);
        out.extend(both.iter().map(|r| single(r.action, r.likelihood)));
    }
    let mut others: Vec<Rated> = allowed
        .iter()
        .filter_map(|a| find(*a))
        .filter(|r| r.likelihood >= floor)
        .copied()
        .collect();
    others.sort_by(likelier);
    // A pair usually done together, as one, first among the others.
    if let Some(&(a, b)) = COMBOS.iter().find(|(a, b)| {
        others.iter().any(|r| r.action == *a) && others.iter().any(|r| r.action == *b)
    }) {
        let likelihood = others
            .iter()
            .filter(|r| r.action == a || r.action == b)
            .map(|r| r.likelihood)
            .fold(1.0_f64, f64::min);
        others.retain(|r| r.action != a && r.action != b);
        out.push(SuggestedAction {
            actions: vec![wire(a), wire(b)],
            likelihood: percent(likelihood),
        });
    }
    out.extend(others.iter().map(|r| single(r.action, r.likelihood)));
    out.truncate(MAX_ACTIONS);
    out
}

fn single(action: ActionId, likelihood: f64) -> SuggestedAction {
    SuggestedAction {
        actions: vec![wire(action)],
        likelihood: percent(likelihood),
    }
}

/// The wire name of `action`.
fn wire(action: ActionId) -> NeedAction {
    match action {
        ActionId::Reply => NeedAction::Reply,
        ActionId::Remind => NeedAction::Remind,
        ActionId::OpenAttachment => NeedAction::OpenAttachment,
        ActionId::Forward => NeedAction::Forward,
        ActionId::Report => NeedAction::Report,
        ActionId::Safe => NeedAction::Safe,
        ActionId::AlreadyPaid => NeedAction::AlreadyPaid,
        ActionId::StillToPay => NeedAction::StillToPay,
    }
}

/// `likelihood` (0 to 1) in whole percent.
fn percent(likelihood: f64) -> u32 {
    // Clamped to 0..=100 first, so the cast loses nothing.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = (likelihood.clamp(0.0, 1.0) * 100.0).round() as u32;
    percent
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOOR: f64 = 0.7;

    fn rated(action: ActionId, likelihood: f64) -> Rated {
        Rated { action, likelihood }
    }

    fn names(ranked: &[SuggestedAction]) -> Vec<Vec<&'static str>> {
        ranked
            .iter()
            .map(|s| s.actions.iter().map(|a| a.as_str()).collect())
            .collect()
    }

    #[test]
    fn a_reply_offers_what_cleared_the_floor_likelier_first() {
        let all = [
            rated(ActionId::Reply, 0.93),
            rated(ActionId::Remind, 0.34),
            rated(ActionId::OpenAttachment, 0.81),
            rated(ActionId::Forward, 0.72),
            rated(ActionId::Report, 0.02),
            rated(ActionId::Safe, 0.98),
            rated(ActionId::AlreadyPaid, 0.9),
        ];
        let ranked = rank(NeedReason::Reply, NeedQuestion::NeedsReply, &all, FLOOR);
        assert_eq!(
            names(&ranked),
            [vec!["reply"], vec!["open_attachment"], vec!["forward"]]
        );
        assert_eq!(ranked[0].likelihood, 93);
        // Below the floor, nothing.
        let low = [rated(ActionId::Reply, 0.4)];
        assert!(rank(NeedReason::Do, NeedQuestion::AsksAction, &low, FLOOR).is_empty());
    }

    #[test]
    fn a_need_s_own_answers_are_always_offered() {
        let phishing = [
            rated(ActionId::Report, 0.2),
            rated(ActionId::Safe, 0.8),
            rated(ActionId::Reply, 0.95),
        ];
        // Both answers, likelier first, and nothing else on a warning.
        assert_eq!(
            names(&rank(
                NeedReason::Review,
                NeedQuestion::Suspicious,
                &phishing,
                FLOOR
            )),
            [vec!["safe"], vec!["report"]]
        );
        let bill = [
            rated(ActionId::AlreadyPaid, 0.1),
            rated(ActionId::StillToPay, 0.9),
            rated(ActionId::OpenAttachment, 0.85),
            rated(ActionId::Remind, 0.75),
        ];
        assert_eq!(
            names(&rank(NeedReason::Pay, NeedQuestion::Money, &bill, FLOOR)),
            [
                vec!["still_to_pay"],
                vec!["already_paid"],
                vec!["open_attachment"]
            ]
        );
    }

    #[test]
    fn a_need_without_actions_reads_exactly_as_before() {
        use crate::{
            domain::needs::NeedView,
            model::{NeedBasis, NeedTrust},
        };
        let need = NeedView {
            reason: NeedReason::Reply,
            trust: NeedTrust::Sure,
            basis: NeedBasis::Model,
            question: NeedQuestion::NeedsReply,
            rule: None,
            due_on: None,
            overdue: false,
            lapses_at: None,
            urgent: false,
            message_id: MessageId::new(),
            sent_at: None,
            quote: None,
            actions: None,
            waiting_on: None,
            nudges: None,
        };
        let json = serde_json::to_value(&need).unwrap();
        let keys: std::collections::BTreeSet<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        // The fields every need had before the catalogue (and the dates of
        // campbooks-cloud#275), and no other.
        assert_eq!(
            keys,
            [
                "basis",
                "due_on",
                "lapses_at",
                "message_id",
                "overdue",
                "question",
                "quote",
                "reason",
                "rule",
                "sent_at",
                "trust",
                "urgent"
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn nothing_stored_offers_nothing() {
        assert!(rank(NeedReason::Pay, NeedQuestion::Money, &[], FLOOR).is_empty());
        assert!(rank(NeedReason::Review, NeedQuestion::Suspicious, &[], FLOOR).is_empty());
    }
}
