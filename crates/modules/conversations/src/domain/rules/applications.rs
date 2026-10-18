//! What binds a rule's application (a row of
//! `conversations.rule_applications`: what a rule changed on one
//! conversation, and what it was before) to the rule that made it
//! (campbooks-cloud#242).
//!
//! Putting a rule's changes back reads its applications and asks the
//! provider to undo what each one says: take mail out of the spam or the
//! trash, back into the inbox, labels off, back to its stream. A transaction
//! working in the workspace could insert one, or rewrite one's `before`,
//! and the next undo would take a phishing mail out of spam. So an
//! application carries a MAC under the kernel's [`Binding`] with its own
//! context label, over:
//!
//! - the workspace, the rule, the rule's inbox and the conversation (the
//!   row's identity, and where it may act);
//! - the run that made it, if any (none for mail arriving);
//! - its `before`, as [`Before`] encodes it: what the conversation was (its
//!   stream and who put it there) and what the rule did (where it moved it,
//!   whether it archived it or marked it read, its labels, the questions it
//!   answered, where it binned which messages, whether it was archived then).
//!
//! It is signed only in the effect path (`effects::record`), right
//! after the rule verified (`rules::acting` on arrival and in a run), from
//! what that path did and, when the conversation already held changes of
//! the rule, what the existing row recorded, only when that row verifies
//! too. The row is claimed and locked before it is read ([`claim`]), the
//! two are merged in Rust ([`merge`]) and written with the MAC in one
//! statement ([`write`]), so no MAC ever covers what SQL wrote. Putting back (`apply::revert`) verifies each row under its
//! lock and leaves one that does not verify as it is.

use campbooks_kernel::{
    Error, Result,
    binding::{Binding, Fields},
};
use serde_json::Value;
use sqlx::{PgConnection, types::Uuid};

use super::Before;
use crate::{
    domain::redact,
    model::{ConversationId, InboxId, RuleId},
};

/// What a rule changed on one conversation, as its owner recorded it.
const APPLICATION: &str = "campbooks/conversations.rule-application/v1";

/// Whose application it is: the workspace, the rule and the rule's inbox.
pub type Owner = (Uuid, InboxId, RuleId);

/// A locked row: its `before`, run, MAC, and whether it is not put back.
type Locked = (Value, Option<Uuid>, Option<Vec<u8>>, bool);

/// The fields an application's MAC covers, or `None` when its `before`
/// does not encode.
fn fields(
    owner: Owner,
    conversation: ConversationId,
    run: Option<Uuid>,
    before: &Before,
) -> Option<Fields> {
    let (workspace, inbox, rule) = owner;
    let before = serde_json::to_vec(before).ok()?;
    let mut fields = Fields::new(APPLICATION);
    fields
        .uuid(&workspace)
        .uuid(rule.as_uuid())
        .uuid(inbox.as_uuid())
        .uuid(conversation.as_uuid())
        .opt_uuid(run.as_ref())
        .bytes(&before);
    Some(fields)
}

/// The stored row of `owner`'s application to `conversation`: its
/// `before`, run and MAC.
async fn row(
    conn: &mut PgConnection,
    owner: Owner,
    conversation: ConversationId,
) -> Result<Option<(Value, Option<Uuid>, Option<Vec<u8>>)>> {
    Ok(sqlx::query_as(
        "SELECT before, run_id, mac FROM conversations.rule_applications
          WHERE rule_id = $1 AND conversation_id = $2",
    )
    .bind(owner.2.as_uuid())
    .bind(conversation.as_uuid())
    .fetch_optional(conn)
    .await?)
}

/// Whether `before`, read from the stored row of `owner`'s application to
/// `conversation` (made in `run`) with `mac`, is bound to the rule: its MAC
/// verifies over it. A row with no MAC (planted with SQL, or written before
/// applications were bound), another row's MAC, or a row changed since, is
/// not.
pub fn verifies(
    binding: &Binding,
    owner: Owner,
    conversation: ConversationId,
    run: Option<Uuid>,
    stored: (&Before, Option<&[u8]>),
) -> bool {
    let (before, mac) = stored;
    fields(owner, conversation, run, before).is_some_and(|f| binding.verify(&f, mac))
}

/// What `stored` (the row of `owner`'s application to `conversation`)
/// recorded, when it is bound to the rule; `None` when it does not decode
/// or verify.
fn checked(
    binding: &Binding,
    owner: Owner,
    conversation: ConversationId,
    stored: (Value, Option<Uuid>, Option<Vec<u8>>),
) -> Option<Before> {
    let (before, run, mac) = stored;
    let before = serde_json::from_value::<Before>(before).ok()?;
    verifies(binding, owner, conversation, run, (&before, mac.as_deref())).then_some(before)
}

/// `before`, just read (locked) from the row of `owner`'s application to
/// `conversation`, when that row is bound to the rule; `None` when there is
/// no such row, or it does not decode or verify.
pub async fn bound(
    conn: &mut PgConnection,
    binding: &Binding,
    owner: Owner,
    conversation: ConversationId,
    before: Value,
) -> Result<Option<Before>> {
    Ok(row(conn, owner, conversation)
        .await?
        .and_then(|(_, run, mac)| checked(binding, owner, conversation, (before, run, mac))))
}

/// What the effect path found in `owner`'s application to a conversation,
/// with the row claimed and locked ([`claim`]).
pub struct Held {
    /// Whether the rule holds the conversation changed: a row it has not
    /// put back.
    pub holds: bool,
    /// What the row recorded and the run that made it, only when the row is
    /// bound to the rule.
    pub bound: Option<(Before, Option<Uuid>)>,
}

/// Claim and lock the row of `owner`'s application to `conversation` before
/// the effect path reads it: insert an empty one if there is none (so it is
/// ours), else lock the existing one. A row another transaction is
/// inserting is waited for, then read as it committed. What it recorded is
/// kept only if it is bound to the rule; a row that is not (planted, edited
/// or written before applications were bound) is dropped with a WARN, so no
/// MAC ever covers what SQL wrote.
pub async fn claim(
    conn: &mut PgConnection,
    binding: &Binding,
    owner: Owner,
    conversation: ConversationId,
) -> Result<Held> {
    let (workspace, _, rule) = owner;
    let claimed: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO conversations.rule_applications (rule_id, conversation_id, workspace_id, before)
         VALUES ($1, $2, $3, '{}'::jsonb)
         ON CONFLICT (rule_id, conversation_id) DO NOTHING
         RETURNING rule_id",
    )
    .bind(rule.as_uuid())
    .bind(conversation.as_uuid())
    .bind(workspace)
    .fetch_optional(&mut *conn)
    .await?;
    let empty = Held {
        holds: false,
        bound: None,
    };
    if claimed.is_some() {
        return Ok(empty);
    }
    let row: Option<Locked> = sqlx::query_as(
        "SELECT before, run_id, mac, reverted_at IS NULL FROM conversations.rule_applications
          WHERE rule_id = $1 AND conversation_id = $2 FOR UPDATE",
    )
    .bind(rule.as_uuid())
    .bind(conversation.as_uuid())
    .fetch_optional(conn)
    .await?;
    let Some((before, run, mac, holds)) = row else {
        return Ok(empty);
    };
    let bound = checked(binding, owner, conversation, (before, run, mac));
    if bound.is_none() {
        tracing::warn!(
            %workspace,
            %rule,
            %conversation,
            "an application of a taught rule is not bound to it; what it recorded is dropped"
        );
    }
    Ok(Held {
        holds,
        bound: bound.map(|before| (before, run)),
    })
}

/// `old`, what a bound application recorded, with what the rule did now:
/// what the conversation was the first time (its stream, where it binned
/// it), where the rule moved it last, and everything it did either time.
pub fn merge(old: Before, now: &Before) -> Before {
    let union = |mut first: Vec<String>, second: &[String]| {
        for item in second {
            if !first.contains(item) {
                first.push(item.clone());
            }
        }
        first
    };
    let mut labels = old.labels;
    for label in &now.labels {
        if !labels.contains(label) {
            labels.push(*label);
        }
    }
    Before {
        stream: old.stream.or_else(|| now.stream.clone()),
        moved_to: now.moved_to.or(old.moved_to),
        archived: old.archived || now.archived,
        read: old.read || now.read,
        labels,
        answered: union(old.answered, &now.answered),
        binned: old.binned.or(now.binned),
        binned_messages: union(old.binned_messages, &now.binned_messages),
        binned_archived: old.binned_archived || now.binned_archived,
    }
}

/// Write `before` (made in `run`) as `owner`'s application to
/// `conversation`, with its MAC, in one statement: the row [`claim`] locked
/// is overwritten whole, so the value signed is the one computed here.
/// Without a key (no instance secret; no rule acts then) it is stored
/// unsigned, and nothing puts it back.
pub async fn write(
    conn: &mut PgConnection,
    binding: &Binding,
    owner: Owner,
    at: (ConversationId, Option<Uuid>),
    before: &Before,
) -> Result<()> {
    let (workspace, _, rule) = owner;
    let (conversation, run) = at;
    let mac = fields(owner, conversation, run, before).and_then(|fields| binding.tag(&fields));
    let json = serde_json::to_value(before).map_err(|e| Error::internal(redact::json(&e)))?;
    sqlx::query(
        "INSERT INTO conversations.rule_applications
             (rule_id, conversation_id, workspace_id, run_id, before, mac)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (rule_id, conversation_id) DO UPDATE
            SET run_id = EXCLUDED.run_id, before = EXCLUDED.before, mac = EXCLUDED.mac,
                applied_at = now(), reverted_at = NULL",
    )
    .bind(rule.as_uuid())
    .bind(conversation.as_uuid())
    .bind(workspace)
    .bind(run)
    .bind(json)
    .bind(mac)
    .execute(conn)
    .await?;
    Ok(())
}

/// How many conversations `rule` holds changed whose application has no MAC:
/// written before applications were bound (or planted so), they cannot be
/// put back.
pub async fn unsigned_held(conn: &mut PgConnection, rule: RuleId) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM conversations.rule_applications
          WHERE rule_id = $1 AND reverted_at IS NULL AND excepted_at IS NULL AND mac IS NULL",
    )
    .bind(rule.as_uuid())
    .fetch_one(conn)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LabelId, Stream};

    fn binding() -> Binding {
        campbooks_kernel::testing::binding()
    }

    fn before() -> Before {
        Before {
            stream: Some((Stream::Primary, "default".into())),
            moved_to: Some(Stream::Updates),
            archived: true,
            labels: vec![LabelId::new()],
            ..Before::default()
        }
    }

    fn owner() -> Owner {
        (Uuid::now_v7(), InboxId::new(), RuleId::new())
    }

    fn tag(owner: Owner, conversation: ConversationId, run: Option<Uuid>, b: &Before) -> Vec<u8> {
        binding()
            .tag(&fields(owner, conversation, run, b).unwrap())
            .unwrap()
    }

    #[test]
    fn an_application_is_bound_only_as_it_was_signed() {
        let owner = owner();
        let conversation = ConversationId::new();
        let run = Some(Uuid::now_v7());
        let before = before();
        let mac = tag(owner, conversation, run, &before);
        let signed = Some(mac.as_slice());
        let holds = |who: Owner, at: ConversationId, made_in: Option<Uuid>, what: &Before| {
            verifies(&binding(), who, at, made_in, (what, signed))
        };
        assert!(holds(owner, conversation, run, &before));
        // Its `before` rewritten: what it was, or what the rule did.
        let mut out_of_spam = before.clone();
        out_of_spam.binned = Some(campbooks_ai::decide::taught::Bin::Spam);
        out_of_spam.binned_messages = vec!["phish-1".into()];
        let mut stream = before.clone();
        stream.stream = Some((Stream::Primary, "member".into()));
        let mut unarchived = before.clone();
        unarchived.archived = false;
        let mut labels = before.clone();
        labels.labels.push(LabelId::new());
        for changed in [out_of_spam, stream, unarchived, labels, Before::default()] {
            assert!(!holds(owner, conversation, run, &changed));
        }
        // Copied onto another conversation, rule, inbox, workspace or run.
        let (workspace, inbox, rule) = owner;
        assert!(!holds(owner, ConversationId::new(), run, &before));
        assert!(!holds(
            (workspace, inbox, RuleId::new()),
            conversation,
            run,
            &before
        ));
        assert!(!holds(
            (workspace, InboxId::new(), rule),
            conversation,
            run,
            &before
        ));
        assert!(!holds(
            (Uuid::now_v7(), inbox, rule),
            conversation,
            run,
            &before
        ));
        assert!(!holds(owner, conversation, None, &before));
        assert!(!holds(owner, conversation, Some(Uuid::now_v7()), &before));
        // No MAC, a flipped bit, or another instance's key.
        let check = |key: &Binding, tag: Option<&[u8]>| {
            verifies(key, owner, conversation, run, (&before, tag))
        };
        assert!(!check(&binding(), None));
        let mut flipped = mac.clone();
        flipped[7] ^= 1;
        assert!(!check(&binding(), Some(flipped.as_slice())));
        assert!(!check(&Binding::disabled(), signed));
    }

    #[test]
    fn a_change_again_keeps_the_first_before_and_adds_what_the_rule_did() {
        let label = LabelId::new();
        let old = Before {
            stream: Some((Stream::Primary, "default".into())),
            moved_to: Some(Stream::Updates),
            labels: vec![label],
            binned: Some(campbooks_ai::decide::taught::Bin::Spam),
            binned_messages: vec!["m-1".into()],
            ..Before::default()
        };
        let other = LabelId::new();
        let now = Before {
            stream: Some((Stream::Updates, "taught".into())),
            moved_to: Some(Stream::Newsletters),
            read: true,
            labels: vec![label, other],
            answered: vec!["needs_reply".into()],
            binned: Some(campbooks_ai::decide::taught::Bin::Trash),
            binned_messages: vec!["m-1".into(), "m-2".into()],
            ..Before::default()
        };
        let merged = merge(old, &now);
        assert_eq!(merged.stream, Some((Stream::Primary, "default".into())));
        assert_eq!(merged.moved_to, Some(Stream::Newsletters));
        assert!(merged.read && !merged.archived);
        assert_eq!(merged.labels, [label, other]);
        assert_eq!(merged.answered, ["needs_reply"]);
        assert_eq!(merged.binned, Some(campbooks_ai::decide::taught::Bin::Spam));
        assert_eq!(merged.binned_messages, ["m-1", "m-2"]);
    }

    #[test]
    fn a_stored_before_parses_back_to_the_same_fields() {
        let owner = owner();
        let conversation = ConversationId::new();
        let before = before();
        let mac = tag(owner, conversation, None, &before);
        // As jsonb returns it: keys in another order.
        let mut stored = serde_json::to_value(&before).unwrap();
        let object = stored.as_object_mut().unwrap();
        let labels = object.remove("labels").unwrap();
        object.insert("labels".into(), labels);
        let parsed: Before = serde_json::from_value(stored).unwrap();
        assert!(verifies(
            &binding(),
            owner,
            conversation,
            None,
            (&parsed, Some(mac.as_slice()))
        ));
    }
}
