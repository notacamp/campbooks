//! What binds a taught rule to the member who taught it
//! (campbooks-cloud#242): a MAC under the kernel's [`Binding`] (the key of
//! campbooks-cloud#227, whose key the database never holds), with its own
//! context label.
//!
//! A row of `conversations.rules` is no proof that a member taught it: a
//! transaction working in the workspace can insert one, or change an
//! existing one's conditions, actions or inbox, and the next inbound mail
//! would run it. So a rule acts only when its MAC verifies over everything
//! that decides what it matches and what it does ([`Stored::bound`]):
//!
//! - the workspace, the rule, its binding generation (one more at every
//!   signing), its inbox, who taught it (whose rights it acts with) and its
//!   stored state (`active` or `paused`; a proposal is never signed);
//! - its precedence (`confirmed_at`, or `created_at`: the later confirmed of
//!   two equally specific rules wins a slot);
//! - a canonical hash of its conditions and actions ([`content_hash`]):
//!   every condition (sender, domain, list, subject, header, label, stream,
//!   failed checks) and every action (stream, archive, label, read, the
//!   answers, trash, spam, paid, showing images). The specificity is
//!   computed from the conditions, so it is covered too.
//!
//! A rule is signed only inside an authenticated member's request
//! ([`sign`]): creating it active, confirming it (a proposal, or a rule
//! that needs confirming again), changing it, pausing, resuming, adopting,
//! removing or restoring it. A request that keeps any bound field it did
//! not set itself (a pause, an adopt, a change of only the actions)
//! verifies the rule first, under its row lock, and refuses (409
//! `rule_needs_confirmation`) or signs nothing when it does not verify. No
//! job, consumer or startup task signs a rule, except the one-off
//! re-signing of the rules that existed when the binding was added, and only
//! when their content still hashes to what the migration snapshotted
//! (`features::resign_rules`).

use std::collections::HashMap;

use campbooks_kernel::{
    Error, Result,
    binding::{Binding, Fields},
    tenancy::{Access, UserId},
};
use sqlx::{PgConnection, types::Uuid};

use super::{RuleAction, RuleCondition, Stored, find_any};
use crate::{
    domain::{
        binding::{ContentToken, Shown},
        redact,
    },
    model::{RuleId, RuleState},
};

/// The encoding of a rule's conditions and actions. A new field takes a new
/// version.
const RULE_CONTENT: &str = "campbooks/conversations.rule-content/v1";
/// A taught rule, as a member left it.
const RULE: &str = "campbooks/conversations.rule/v1";

/// SHA-256 of a rule's conditions then actions, in order, each its typed
/// value serialised (the field order of its type, so a stored copy that
/// parses back encodes the same), length-prefixed with a count per list.
pub fn content_hash(conditions: &[RuleCondition], actions: &[RuleAction]) -> Result<[u8; 32]> {
    let json = |e: serde_json::Error| Error::internal(redact::json(&e));
    let mut fields = Fields::new(RULE_CONTENT);
    fields.count(conditions.len());
    for condition in conditions {
        fields.bytes(&serde_json::to_vec(condition).map_err(json)?);
    }
    fields.count(actions.len());
    for action in actions {
        fields.bytes(&serde_json::to_vec(action).map_err(json)?);
    }
    Ok(fields.digest())
}

impl Stored {
    /// The rule's bound fields, at `generation`.
    fn bound_fields(&self, generation: i64) -> Result<Fields> {
        let content = content_hash(&self.conditions, &self.actions)?;
        let precedence = self
            .confirmed_at
            .unwrap_or(self.created_at)
            .timestamp_micros();
        let mut fields = Fields::new(RULE);
        fields
            .uuid(&self.workspace)
            .uuid(self.id.as_uuid())
            .int(generation)
            .uuid(self.inbox.as_uuid())
            .opt_uuid(self.taught_by.as_ref().map(UserId::as_uuid))
            .str(self.state.as_str())
            .int(precedence)
            .bytes(&content);
        Ok(fields)
    }

    /// Whether the rule's MAC verifies over it as stored: a member taught it
    /// as it is. A proposal never is (it acts only once confirmed), nor is a
    /// rule with no MAC, another rule's, or one whose fields changed since.
    pub fn bound(&self, binding: &Binding) -> bool {
        self.state != RuleState::Proposed
            && self
                .bound_fields(self.generation)
                .is_ok_and(|fields| binding.verify(&fields, self.mac.as_deref()))
    }

    /// The rule's MAC under the current key, when its stored one verifies
    /// under the previous key only (a rotation of the instance secret key,
    /// campbooks-cloud#191): at the same generation, so the content tokens
    /// members hold stay valid. `None` for a proposal, a rule bound under
    /// the current key already, or one that does not verify.
    pub fn retag(&self, binding: &Binding) -> Option<Vec<u8>> {
        if self.state == RuleState::Proposed {
            return None;
        }
        self.bound_fields(self.generation)
            .ok()
            .and_then(|fields| binding.retag(&fields, self.mac.as_deref()))
    }

    /// Whether a member must confirm the rule again before it acts: it is
    /// kept (active or paused) but not [`bound`](Self::bound).
    pub fn needs_confirmation(&self, binding: &Binding) -> bool {
        matches!(self.state, RuleState::Active | RuleState::Paused) && !self.bound(binding)
    }
}

/// The token of `rule` as it is stored now, shown to the caller of
/// `access` (campbooks-cloud#242): over the same [`content_hash`] its MAC
/// binds, with its id, binding generation, inbox, stored state and author.
/// `conversations.confirm_rule` (and a manager adopting it) signs it only
/// when the member sends this token back and it still holds.
pub fn content_token(access: &Access, rule: &Stored) -> Result<ContentToken> {
    Ok(ContentToken::for_caller(
        access,
        Shown::Rule {
            id: rule.id,
            generation: rule.generation,
            inbox: rule.inbox,
            state: rule.state,
            taught_by: rule.taught_by,
            content: content_hash(&rule.conditions, &rule.actions)?,
        },
    ))
}

/// Fill in the binding columns of `rules` (`workspace_id`,
/// `binding_generation`, `mac`), read in the same transaction.
pub(super) async fn attach(conn: &mut PgConnection, rules: &mut [Stored]) -> Result<()> {
    if rules.is_empty() {
        return Ok(());
    }
    let ids: Vec<Uuid> = rules.iter().map(|r| *r.id.as_uuid()).collect();
    let rows: Vec<(Uuid, Uuid, i64, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT id, workspace_id, binding_generation, mac FROM conversations.rules
          WHERE id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(conn)
    .await?;
    let mut rows: HashMap<Uuid, (Uuid, i64, Option<Vec<u8>>)> = rows
        .into_iter()
        .map(|(id, workspace, generation, mac)| (id, (workspace, generation, mac)))
        .collect();
    for rule in rules.iter_mut() {
        if let Some((workspace, generation, mac)) = rows.remove(rule.id.as_uuid()) {
            rule.workspace = workspace;
            rule.generation = generation;
            rule.mac = mac;
        }
    }
    Ok(())
}

/// Sign rule `id` as it is now stored, at the next generation.
///
/// Only inside an authenticated member's request, with the rule's row
/// locked by that request (written or `FOR UPDATE`), and only when every
/// bound field is the member's: set by the request itself, or verified
/// ([`Stored::bound`]) under that lock before the request changed it. A
/// proposal is not signed. Without a key (no instance secret) nothing can be
/// bound, so the request fails (500) rather than leave a rule that would
/// never act.
pub async fn sign(conn: &mut PgConnection, binding: &Binding, id: RuleId) -> Result<()> {
    let Some(rule) = find_any(conn, id).await? else {
        return Err(Error::NotFound);
    };
    if rule.state == RuleState::Proposed {
        return Ok(());
    }
    let generation = rule.generation.saturating_add(1);
    let Some(mac) = binding.tag(&rule.bound_fields(generation)?) else {
        tracing::warn!(
            workspace = %rule.workspace,
            rule = %rule.id,
            "a taught rule was refused: there is no key to bind it with \
             (CAMPBOOKS_SECRET_KEY is unset)"
        );
        return Err(Error::internal("rules are not bound: no instance secret"));
    };
    sqlx::query("UPDATE conversations.rules SET binding_generation = $2, mac = $3 WHERE id = $1")
        .bind(id.as_uuid())
        .bind(generation)
        .bind(mac)
        .execute(conn)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{TimeDelta, Utc};

    use super::*;
    use crate::model::{InboxId, LabelId, RuleOrigin, Stream, TeachingId};

    /// A change SQL could make to a stored rule.
    type Tamper = Box<dyn Fn(&mut Stored)>;

    fn binding() -> Binding {
        campbooks_kernel::testing::binding()
    }

    fn rule() -> Stored {
        Stored {
            id: RuleId::new(),
            inbox: InboxId::new(),
            teaching: TeachingId::new(),
            conditions: vec![RuleCondition::SenderAddress("news@example.com".into())],
            actions: vec![RuleAction::Archive],
            origin: RuleOrigin::Action,
            origin_text: None,
            state: RuleState::Active,
            taught_by: Some(UserId::new()),
            from_conversation: None,
            created_at: Utc::now(),
            confirmed_at: Some(Utc::now()),
            applied_count: 0,
            last_applied_at: None,
            author_may_act: true,
            author_suspended: false,
            inbox_detached: false,
            workspace: Uuid::now_v7(),
            generation: 0,
            mac: None,
        }
    }

    fn signed(mut rule: Stored) -> Stored {
        rule.generation += 1;
        rule.mac = binding().tag(&rule.bound_fields(rule.generation).unwrap());
        rule
    }

    #[test]
    fn a_rule_is_bound_only_as_it_was_signed() {
        let rule = signed(rule());
        assert!(rule.bound(&binding()));
        assert_eq!(rule.effective_state(), RuleState::Active);
        assert!(!rule.needs_confirmation(&binding()));
        let other_label = LabelId::new();
        let changed: Vec<Tamper> = vec![
            Box::new(|r| r.actions = vec![RuleAction::Trash]),
            Box::new(|r| r.actions.push(RuleAction::Spam)),
            Box::new(move |r| {
                r.actions = vec![RuleAction::AddLabel { label: other_label }];
            }),
            Box::new(|r| r.actions = vec![RuleAction::ShowImages]),
            Box::new(|r| {
                r.conditions = vec![RuleCondition::SenderDomain("example.com".into())];
            }),
            Box::new(|r| r.conditions.clear()),
            Box::new(|r| r.conditions.push(RuleCondition::Stream(Stream::Primary))),
            Box::new(|r| r.inbox = InboxId::new()),
            Box::new(|r| r.workspace = Uuid::now_v7()),
            Box::new(|r| r.id = RuleId::new()),
            Box::new(|r| r.taught_by = Some(UserId::new())),
            Box::new(|r| r.taught_by = None),
            Box::new(|r| r.generation += 1),
            Box::new(|r| r.state = RuleState::Paused),
            Box::new(|r| r.confirmed_at = r.confirmed_at.map(|c| c + TimeDelta::seconds(1))),
            Box::new(|r| r.mac = None),
            Box::new(|r| {
                if let Some(mac) = r.mac.as_mut() {
                    mac[5] ^= 1;
                }
            }),
        ];
        for (i, change) in changed.iter().enumerate() {
            let mut tampered = rule.clone();
            change(&mut tampered);
            assert!(!tampered.bound(&binding()), "change {i} still verifies");
        }
        // Another instance's key, or none.
        assert!(!rule.bound(&Binding::disabled()));
        let other = Binding::new(&campbooks_kernel::crypto::SecretKey::from_bytes(
            [7; campbooks_kernel::crypto::SECRET_KEY_LEN],
        ));
        assert!(!rule.bound(&other));
    }

    #[test]
    fn a_mac_copied_onto_another_rule_inbox_or_workspace_does_not_verify() {
        let first = signed(rule());
        // Another rule of the same inbox and workspace, with the same
        // content, generation and MAC.
        let mut copy = first.clone();
        copy.id = RuleId::new();
        assert!(!copy.bound(&binding()));
        // The same rule moved to another inbox, or another workspace.
        let mut moved = first.clone();
        moved.inbox = InboxId::new();
        assert!(!moved.bound(&binding()));
        let mut elsewhere = first.clone();
        elsewhere.workspace = Uuid::now_v7();
        assert!(!elsewhere.bound(&binding()));
        // Another member's rule of the same content, given this one's MAC.
        let mut theirs = first.clone();
        theirs.taught_by = Some(UserId::new());
        assert!(!theirs.bound(&binding()));
        assert!(first.bound(&binding()));
    }

    #[test]
    fn a_proposal_or_an_unsigned_rule_never_acts() {
        let mut proposal = signed(rule());
        proposal.state = RuleState::Proposed;
        assert!(!proposal.bound(&binding()));
        assert!(!proposal.needs_confirmation(&binding()));
        let unsigned = rule();
        assert!(!unsigned.bound(&binding()));
        assert!(unsigned.needs_confirmation(&binding()));
        let mut removed = rule();
        removed.state = RuleState::Removed;
        assert!(!removed.needs_confirmation(&binding()));
        // Paused by its author's absence, yet bound: it does not act.
        let mut away = signed(rule());
        away.author_suspended = true;
        assert!(away.bound(&binding()));
        assert_ne!(away.effective_state(), RuleState::Active);
    }

    #[test]
    fn the_content_hash_is_canonical() {
        let a = content_hash(
            &[RuleCondition::SenderAddress("a@example.com".into())],
            &[RuleAction::Archive, RuleAction::MarkRead],
        )
        .unwrap();
        let b = content_hash(
            &[RuleCondition::SenderAddress("a@example.com".into())],
            &[RuleAction::MarkRead, RuleAction::Archive],
        )
        .unwrap();
        let c = content_hash(
            &[],
            &[
                RuleAction::Archive,
                RuleAction::MarkRead,
                RuleAction::Archive,
            ],
        )
        .unwrap();
        assert_ne!(a, b);
        assert_ne!(a, c);
        // A stored copy parses back and hashes the same.
        let stored = serde_json::json!([{"field": "sender_address", "value": "a@example.com"}]);
        let parsed: Vec<RuleCondition> = serde_json::from_value(stored).unwrap();
        assert_eq!(
            content_hash(&parsed, &[RuleAction::Archive, RuleAction::MarkRead]).unwrap(),
            a
        );
    }
}
