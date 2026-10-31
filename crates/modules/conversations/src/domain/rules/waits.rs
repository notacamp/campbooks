//! "Wait on them" (campbooks-cloud#378): the answers a taught rule with
//! [`RuleAction::WaitOnAddressed`](super::RuleAction) gives about one
//! message.
//!
//! The rule answers as a taught source what the member's own choice would
//! have filed: `next_move` is `someone_else` and `awaited` names the person
//! the message was written to. The person is read from the headers alone
//! (ADR 0018) and only when there is exactly one obvious one: the email's
//! single awaitable person in `To`. Zero or several answer nothing, so the
//! card asks as before. The need is then worked out like any wait
//! (`needs::waiting`), with its own guards: never the inbox, never a
//! machine, a nudge after the usual working days, and heard-back clears it.

use campbooks_ai::decide::signals::{
    facts::awaitable,
    moves::{self, NextMove},
};
use campbooks_kernel::Result;
use serde_json::json;
use sqlx::PgConnection;

use super::effects::TaughtAnswer;
use crate::{domain::needs, model::MessageId};

/// The answers `rule` gives about `message`: none when it was written to
/// nobody, or to more than one person, who may be awaited.
pub async fn answers<'a>(
    conn: &mut PgConnection,
    message: MessageId,
    rule: &'a str,
) -> Result<Vec<TaughtAnswer<'a>>> {
    let Some(people) = needs::people(conn, message).await? else {
        return Ok(Vec::new());
    };
    let Some(address) = single_addressed(&people) else {
        return Ok(Vec::new());
    };
    let Some(label) = moves::awaited_label(&people, &address) else {
        return Ok(Vec::new());
    };
    let choices: Vec<&str> = NextMove::ALL.into_iter().map(NextMove::as_str).collect();
    Ok(vec![
        TaughtAnswer {
            question: moves::question::NEXT_MOVE,
            value: json!(NextMove::SomeoneElse.as_str()),
            options: json!(choices),
            rule,
        },
        TaughtAnswer {
            question: moves::question::AWAITED,
            value: json!(label),
            // The person it names, so a later read can tell the headers
            // still name them.
            options: json!([{"label": label, "description": address}]),
            rule,
        },
    ])
}

/// The one person in `To` who may be awaited, lower-cased; `None` when there
/// is none or more than one.
pub fn single_addressed(people: &campbooks_ai::decide::signals::email::Envelope) -> Option<String> {
    let in_to: Vec<String> = awaitable(people)
        .into_iter()
        .filter(|p| {
            people
                .to
                .iter()
                .any(|t| t.address.trim().eq_ignore_ascii_case(p.address.trim()))
        })
        .map(|p| p.address.trim().to_lowercase())
        .collect();
    match in_to.as_slice() {
        [only] => Some(only.clone()),
        _ => None,
    }
}
