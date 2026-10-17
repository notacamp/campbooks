//! Rules on new mail: what `apply_changes` does after the engine's
//! outcome placed a new inbound message.

use std::collections::BTreeSet;

use campbooks_ai::decide::{signals::email::triage, taught::Outcome};
use campbooks_kernel::{EventBus, Result};
use serde_json::json;
use sqlx::{Postgres, Transaction, types::Uuid};

use super::{
    Before,
    apply::answers,
    effects::{self, Effects, TaughtAnswer},
};
use crate::{
    domain::flags::Flag,
    events::{Change, RequestedChange},
    model::{ConversationId, InboxId, LabelId, MessageId, RuleId, Stream},
};

/// A new inbound message, as the rules stage of `apply_changes` sees it.
pub struct Arrival<'a> {
    /// The workspace id.
    pub workspace: Uuid,
    /// The events' workspace key.
    pub ws: &'a str,
    /// The inbox.
    pub inbox: InboxId,
    /// Its conversation.
    pub conversation: ConversationId,
    /// The message.
    pub message: MessageId,
    /// Its provider id.
    pub provider_message_id: &'a str,
    /// Where the conversation was before, when a rule placed it.
    pub placed_from: Option<(Stream, String)>,
}

/// What each contributing rule did, to record.
#[derive(Default)]
struct Ledger(Vec<(String, Before)>);

impl Ledger {
    fn of(&mut self, rule: &str) -> &mut Before {
        let index = self
            .0
            .iter()
            .position(|(r, _)| r == rule)
            .unwrap_or_else(|| {
                self.0.push((rule.to_owned(), Before::default()));
                self.0.len() - 1
            });
        &mut self.0[index].1
    }
}

/// Archive, mark read, trash or report the message that just arrived, as
/// `outcome` says, noting each in `ledger` and `effects`.
async fn flag_on_arrival(
    tx: &mut Transaction<'_, Postgres>,
    arrival: &Arrival<'_>,
    outcome: &Outcome,
    ledger: &mut Ledger,
    effects: &mut Effects,
) -> Result<()> {
    for (rule, flag) in [
        (&outcome.archive, Flag::Archive),
        (&outcome.read, Flag::Read),
    ] {
        let Some(rule) = rule else { continue };
        let messages = effects::flag_message(tx, arrival.message, flag).await?;
        if messages.is_empty() {
            continue;
        }
        if flag == Flag::Archive {
            ledger.of(rule).archived = true;
        } else {
            ledger.of(rule).read = true;
        }
        effects.add(flag.change(), messages, flag.requested());
    }
    if let Some(applied) = &outcome.bin {
        let flag = super::apply::bin_flag(applied.value);
        let messages = effects::flag_message(tx, arrival.message, flag).await?;
        if !messages.is_empty() {
            ledger.of(&applied.rule).binned = Some(applied.value);
            effects.add(flag.change(), messages, flag.requested());
        }
    }
    Ok(())
}

/// Carry out what `outcome` says about a message that just arrived: store
/// the rules' answers as `taught` decisions, archive, mark read, label, trash
/// or report the message (and ask the provider to do the same), and record each rule's
/// application. Returns what changed on the conversation, for the batch to
/// announce.
pub async fn on_arrival(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    arrival: &Arrival<'_>,
    outcome: &Outcome,
) -> Result<BTreeSet<Change>> {
    let mut ledger = Ledger::default();
    let mut effects = Effects::default();
    let at = (arrival.message, arrival.conversation);
    let this_message = || vec![arrival.provider_message_id.to_owned()];
    if let (Some(applied), Some(from)) = (&outcome.stream, &arrival.placed_from) {
        let stream: Stream = applied.value.parse()?;
        let before = ledger.of(&applied.rule);
        before.stream = Some(from.clone());
        before.moved_to = Some(stream);
        let answer = TaughtAnswer {
            question: triage::STREAM,
            value: json!(applied.value),
            options: json!(triage::STREAMS),
            rule: &applied.rule,
        };
        effects::store_decision(tx, arrival.workspace, at, &answer).await?;
        effects.add(
            Change::Stream,
            this_message(),
            RequestedChange::MoveToStream { stream },
        );
    }
    flag_on_arrival(tx, arrival, outcome, &mut ledger, &mut effects).await?;
    for applied in &outcome.labels {
        let Ok(label) = applied.value.parse::<LabelId>() else {
            continue;
        };
        let linked = effects::link_label(
            tx,
            arrival.workspace,
            arrival.inbox,
            arrival.conversation,
            label,
        )
        .await?;
        let Some(provider_key) = linked else { continue };
        ledger.of(&applied.rule).labels.push(label);
        effects.add(
            Change::Labels,
            this_message(),
            RequestedChange::AddLabel {
                label,
                provider_key,
            },
        );
    }
    for rule in outcome.rules() {
        for answer in answers(outcome, rule) {
            effects::store_decision(tx, arrival.workspace, at, &answer).await?;
            ledger.of(rule).answered.push(answer.question.to_owned());
        }
    }
    let kinds: BTreeSet<Change> = effects
        .kinds()
        .iter()
        .copied()
        .filter(|k| *k != Change::Stream)
        .collect();
    effects
        .publish(
            tx,
            events,
            (arrival.ws, arrival.inbox, arrival.conversation),
            false,
        )
        .await?;
    for (rule, before) in ledger.0 {
        let Ok(rule) = rule.parse::<RuleId>() else {
            continue;
        };
        effects::record(
            tx,
            arrival.workspace,
            rule,
            (arrival.conversation, None),
            &before,
        )
        .await?;
    }
    Ok(kinds)
}
