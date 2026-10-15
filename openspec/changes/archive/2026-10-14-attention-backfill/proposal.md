# Proposal

Module: `conversations` (needs). The `attention` module is unchanged: it
projects the needs through its existing consumer.
Tracking: https://github.com/notacamp/campbooks-cloud/issues/123 (attention
follow-ups after campbooks-cloud#39).

## Why

"What needs you" on Home shows only needs for mail that arrived after the
attention module shipped. A conversation's need is worked out when its mail
is stored (`conversations.apply_changes`), after the model pass
(`conversations.decide_message`) and when a member acts on it. Mail stored
before the upgrade was never looked at, so an instance with thousands of
messages and their stored decisions has an almost empty Home.

Replaying `conversations.decide_message` would not help: it skips messages
the model already answered and messages older than the decide window, and it
would call the model again, which must not happen for this.

## What Changes

- **Needs for mail stored before attention, on upgrade.** A migration lists
  every inbox that exists in `conversations.needs_backfills`. A scheduled
  dispatcher (`conversations.dispatch_needs_backfills`, every minute) starts
  each pending inbox's `conversations.backfill_needs`, which pages through
  the inbox's conversations by id, 100 at a time, inside the inbox's
  workspace (row-level security applies), and works out each one's need
  with the same `needs::refresh` live mail uses.
- **From stored answers only.** No model and no rules are asked: the need
  comes from the messages and decisions already stored, settled against
  the conversation's state (replied, archived, trashed, a member's verdict).
  Decisions stored before the sender's sentence was kept carry no
  `sentence`: their needs show no quote, never an invented one, and their
  due dates resolve from the deadline words alone.
- **Bounded to recent mail.** Only conversations whose last message falls
  within `CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS` (default 14), the
  window live needs use, plus any conversation that already has an open
  need (so a need its answers no longer support is withdrawn).
- **Idempotent.** A need that did not change is not announced again, so the
  backfill can run twice, or be restarted by a later migration, and change
  nothing.
- Each change publishes the normal `conversations.need_changed`; the
  attention module projects Home's items through its existing consumer.

## Non-goals

- A setting or an operator command: nothing to do for self-hosters.
- Needs for mail older than the needs window: a two-year-old unanswered
  email does not flood Home (the same rule as a first sync).
- Re-running the rules or the model over old mail, or recovering the
  sender's sentence for decisions stored without one.
- Inboxes created after the upgrade: their needs are raised as their mail
  is stored.
