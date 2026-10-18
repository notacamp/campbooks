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
//! too. A row that does not is dropped first, so no MAC ever covers what
//! SQL wrote. Putting back (`apply::revert`) verifies each row under its
//! lock and leaves one that does not verify as it is.

use campbooks_kernel::{
    Result,
    binding::{Binding, Fields},
};
use serde_json::Value;
use sqlx::{PgConnection, types::Uuid};

use super::Before;
use crate::model::{ConversationId, InboxId, RuleId, RuleRunId};

/// What a rule changed on one conversation, as its owner recorded it.
const APPLICATION: &str = "campbooks/conversations.rule-application/v1";

/// Whose application it is: the workspace, the rule and the rule's inbox.
pub type Owner = (Uuid, InboxId, RuleId);

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

/// Before the effect path merges what it did into an existing row of
/// `owner`'s application to `conversation`: drop what that row recorded
/// (its `before` and run) unless it is bound, so the MAC [`seal`] writes
/// never covers what SQL wrote.
pub async fn drop_unbound(
    conn: &mut PgConnection,
    binding: &Binding,
    owner: Owner,
    conversation: ConversationId,
) -> Result<()> {
    let Some(stored) = row(conn, owner, conversation).await? else {
        return Ok(());
    };
    if checked(binding, owner, conversation, stored).is_some() {
        return Ok(());
    }
    let (workspace, _, rule) = owner;
    tracing::warn!(
        %workspace,
        %rule,
        %conversation,
        "an application of a taught rule is not bound to it; what it recorded is dropped"
    );
    sqlx::query(
        "UPDATE conversations.rule_applications SET before = '{}'::jsonb, run_id = NULL, mac = NULL
          WHERE rule_id = $1 AND conversation_id = $2",
    )
    .bind(rule.as_uuid())
    .bind(conversation.as_uuid())
    .execute(conn)
    .await?;
    Ok(())
}

/// Sign `owner`'s application to `conversation` as the effect path just
/// stored it. Only there, right after the rule verified, and only over what
/// that path did and what [`drop_unbound`] kept. Without a key (no instance
/// secret; no rule acts then) or a `before` that does not decode, it stays
/// unsigned, and nothing puts it back. Never an error ingest would fail on.
pub async fn seal(
    conn: &mut PgConnection,
    binding: &Binding,
    owner: Owner,
    conversation: ConversationId,
) -> Result<()> {
    let Some((before, run, _)) = row(conn, owner, conversation).await? else {
        return Ok(());
    };
    let mac = serde_json::from_value::<Before>(before)
        .ok()
        .and_then(|before| fields(owner, conversation, run, &before))
        .and_then(|fields| binding.tag(&fields));
    sqlx::query(
        "UPDATE conversations.rule_applications SET mac = $3
          WHERE rule_id = $1 AND conversation_id = $2",
    )
    .bind(owner.2.as_uuid())
    .bind(conversation.as_uuid())
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
