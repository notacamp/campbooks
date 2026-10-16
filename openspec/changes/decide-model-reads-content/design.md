# Design

## Data ownership and events

- `conversations` owns everything stored: `conversations.decisions` (answers,
  unchanged shape), `conversations.conversations` (`stream`, `stream_source`
  already allows `model`), `conversations.needs`,
  `conversations.needs_backfills`. No new table.
- Events: unchanged. The model pass placing a stream publishes what
  `apply_changes` publishes when it sorts a conversation; needs publish
  `conversations.need_changed` as today.
- No endpoint is added or changed; no new scope.

## AI use

- Role: `decide` only. Provider: Laya (`/systemone`) or Jev through
  OpenRouter (`/api/alpha/decisions`), both `PROTOCOL=laya`, told apart only
  by `BASE_URL`, `PATH` and `MODEL`. An OpenAI-compatible instruct model stays
  valid.
- Fallback when `decide` is not configured, or has not passed its answers
  check: the facts strategy alone. Conversations stay in Primary, the only
  acted answers are the facts' "no" to `needs_reply`, and no sure need is
  raised. Nothing is guessed from text.
- Undo: a member moves a conversation to another stream (the move sticks and
  is never re-sorted); a need is dismissed or confirmed as today.
- Content leaves the instance only with
  `CAMPBOOKS_AI__DECIDE__EXTERNAL_CONTENT=allow` (unchanged gate).

## The facts strategy

`crates/ai/src/decide/signals/facts.rs`:

- `Facts::of(&Envelope)` copies only `from`, `to`, `cc`, the headers
  `List-Id`, `List-Unsubscribe`, `Precedence`, `Auto-Submitted`,
  `Authentication-Results`, `Reply-To`, and attachment media types. It is the
  only code in the file that touches an `Envelope`.
- `EmailFacts` (a `Strategy`) answers `needs_reply` false, confidence 0.99,
  with one reason, when:
  - `facts.list_mail`: `List-Id` or `List-Unsubscribe` present, or
    `Precedence` is `bulk`, `list` or `junk`;
  - `facts.automatic`: `Auto-Submitted` present and not `no`;
  - `facts.no_reply_sender`: the sender's local part is a no-reply address
    (`noreply`, `no-reply`, `donotreply`, `do-not-reply`, `mailer-daemon`,
    `postmaster`, `bounce`, `bounces`, optionally followed by `+`, `.`, `-`
    or `_` and more).
  It abstains on every other question and when no fact applies. It never
  answers true.
- Hints for the model's context (`inbox::preamble`), from facts only: list
  mail (with the `List-Id`), sent automatically, a no-reply address, a
  service-like address (the old `AUTOMATED_LOCAL` pattern on the local part
  only), a `Reply-To` on another domain, and the receiving provider's
  authentication failing (`dmarc=fail`, or no passing DKIM aligned with the
  From domain when results exist). `CONTEXT_VERSION` becomes 2, so
  `STATE_VERSION` becomes 3.
- A unit test reads the file's source and fails if it mentions an
  envelope's `subject`, `body` or an attachment's `name` outside `Facts::of`'s
  field list, so content cannot come back through this module.

`signals/email.rs` keeps `Envelope`, `Party`, `Attachment`, `Recipient`, the
triage constants, `triage_questions()`, `decision_state()` (the model still
reads the subject and the start of the body) and `embedding_text()`. Every
content regex, phrase list, `Read`, `conf`, `QUOTED_RULES`,
`quoted_sentence`, `SECURITY_PLATFORMS` and `signals/dates.rs` are deleted.
`auth_results.rs` stays (facts).

## Policy

`Policy::baseline` is pinned to the committed eval report, as now, with two
runs: `facts` (offline) and `jev` (online, `typesafe/jev-1.13`).

- For `facts`, the `needs_reply` switch is `ActAbove(threshold)` when the
  run holds the target there; every other facts entry is `Record`.
- For the model (`Source::Model`), each question's switch is
  `ActAbove(threshold)` where the `jev` run holds its target on `test`, else
  `Record`.
- The pinning test checks both runs and no longer requires every run to be
  offline. `cargo xtask eval decide --offline --check` compares offline runs
  only and ignores online runs in the baseline.
- `accepted` (needs) keeps "the rules' acted answer wins": a facts "no"
  beats a model "yes".

## The model pass places streams

`conversations.decide_message`, after storing the model's answers: when the
policy accepts the model's `stream` answer, the message is the
conversation's first inbound message, and the conversation's
`stream_source` is still `default`, it sets the stream with source `model`
and stores the stream answer as acted. The sorting code shared with
`apply_changes` (`sort_first_inbound`) moves to `domain/` so both use it.
A member's move (`member`) and an earlier placement are never overridden.

## Needs

`raise`:

- `pay`: `asks_action` accepted and true, and the message's accepted stream
  answer is `receipts` (from any source).
- `reply`: `needs_reply` accepted and true.
- `do`: `asks_action` accepted and true otherwise.
- `review` for `suspicious`, and the `check` question from an unaccepted
  confident model answer, as today.
- `due_on` is always `None`; `urgent` from an accepted `urgency` of 2.
- `PAYMENT_DUE`, `DEADLINE` and `resolve_date` go.

## Upgrade

Migration `<timestamp>_conversations__facts_and_model.sql`:

1. `UPDATE conversations.decisions SET acted = false WHERE source = 'rules'
   AND state_version < 3` — the old rules' answers stay as a record, and stop
   counting.
2. Every inbox is (re)queued in `conversations.needs_backfills`
   (`state = 'pending'`, cursor reset), so `backfill_needs` refreshes each
   recent conversation and withdraws needs the old answers raised.

`backfill_needs` additionally, when a trusted `decide` model is configured,
enqueues the model pass for each conversation's newest inbound message sent
within `CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` that has no checked
model answer (a job carrying ids only, reusing the `decide_message` logic;
idempotent per message).

## Endpoint path

`ProviderConfig` gains `path: Option<String>` (`CAMPBOOKS_AI__<ROLE>__PATH`,
decide only, validated like `PROTOCOL`). `Gateway::decide_with` posts to
`path` when set, else `systemone` (Laya protocol) or `chat/completions`
(OpenAI). The eval harness gains a `jev` strategy: `CAMPBOOKS_EVAL_JEV_URL`
(default `https://openrouter.ai/api/alpha`), `_PATH` (default `decisions`),
`_MODEL` (default `typesafe/jev-1.13`), `_API_KEY`.

## Reasons shown to members

New client keys `reasons.facts.list_mail`, `reasons.facts.automatic`,
`reasons.facts.no_reply_sender` in the four locales. The old `signals.*`
keys stay, so answers stored before the upgrade still read.
