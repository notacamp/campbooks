# Design

## Every model call, found

What calls a model today, and how each one is metered:

| Path | Gateway method | Whose | Metered as |
| --- | --- | --- | --- |
| `conversations.decide_message`, the model pass over each inbound message, and `conversations.decide_backlog`, the same pass over a stored message the needs backfill queues (one shared `Pass::run`) | `decide` (through the `Model` strategy) | the message's workspace | workspace scope, feature `conversations.triage` |
| The `decide` answers check (`Gateway::keep_answers_checked`, eight fictional emails per run, every 6 h while it passes, every 15 min while it fails) | `decide` | the instance | instance scope `ai.answers_check`: admitted without a cap and not stored (bounded by its schedule, not by mail) |
| `Knn` (embeddings of a workspace's corrections) | `embed` | | no production caller yet; would be refused until its caller opens a scope |
| `assistant.compile`, a member's sentence turned into rules (`POST /api/v1/w/{workspace}/assistant/compile`) | `chat` (`complete`) | the workspace, a person waiting | interactive workspace scope, feature `assistant.compile` |
| other `chat` (`vision`) | `chat` | | no production caller yet; would be refused until its caller opens a scope |
| `cargo xtask eval` | `decide`, `embed` | | a gateway without a meter, run by an operator against a chosen endpoint |
| `Gateway::health` | none: `GET /models`, no inference | | not a model call |

`attention` and `search` call no model; `system` only reads the gateway's
health. The search index is Postgres full text (ADR 0016), no embeddings.

The seam is the gateway itself: `chat`, `embed` and `decide` are the only
methods that send anything to a model, and all three go through one
metering step. A metered gateway refuses a call that does not say whose it
is, so the next feature that calls a model fails its first test until it
opens a scope, instead of running uncapped.

## How a call is metered

```
caller                     gateway                       meter (system)
  │ Scope::workspace(db, ws, feature).run(…)
  │ ─────────────────────▶ admit(ws, role, feature) ───▶ upsert day row WHERE calls < cap
  │                        ◀── Admitted{day} | Refused ◀─┘  (+ feature row, one transaction)
  │                        send to the provider (or not)
  │                        settle(day, tokens, failed) ─▶ update feature row (only if tokens or failed)
  │ ◀── answer | OverDailyCap | Metering | Unattributed
```

- **The scope** (`campbooks_ai::usage::Scope`) is a task-local value set
  around the future that ends in the call. It carries the workspace, the
  feature name and the database pool the meter writes with. The strategy
  interface (`Strategy::decide(envelope, questions)`) does not change, so
  rules, k-NN, cascades and the model keep one shape and the eval tool keeps
  working. Task-locals do not cross `tokio::spawn`; the gateway never spawns
  a model call, and a call made from a spawned task without its own scope is
  refused, not uncapped.
- **The meter** (`campbooks_ai::usage::Meter`) is a port: `admit` before the
  call, `settle` after it. The `system` module implements it
  (`campbooks_system::AiUsageMeter`); the server installs it with
  `Gateway::with_meter`. A gateway without a meter (tests, `xtask eval`)
  needs no scope. A server test checks that the composed gateway is metered.
- **One logical call counts once.** `decide`'s retries after a connection
  failure or a 5xx are one call: they are not billed, and counting them
  would let a provider outage eat the day's cap.

## Data ownership

The `system` module owns the ledger, in schema `system` (migration
`<ts>_system__ai_usage.sql`). No new module, no cross-module dependency: the
`ai` crate defines the port, `system` implements it, the server wires them.

```sql
system.ai_usage_days (workspace_id, day, calls, refused, capped_at)        -- PK (workspace_id, day)
system.ai_usage      (workspace_id, day, role, feature, calls, failed,
                      prompt_tokens, completion_tokens)                    -- PK (workspace_id, day, role, feature)
```

Both are workspace tables under row-level security (`workspace_isolation`),
written in `begin_for(workspace)`, with an `admin_reads` `FOR SELECT` policy:
they hold counts, never content. `day` is a UTC date.

**Admission** is one statement on the day row, in the same transaction as
the feature row's `+1`:

```sql
INSERT INTO system.ai_usage_days (workspace_id, day, calls) VALUES ($1, $2, 1)
ON CONFLICT (workspace_id, day) DO UPDATE SET calls = ai_usage_days.calls + 1
 WHERE $3::int IS NULL OR ai_usage_days.calls < $3
RETURNING calls
```

No row back means the cap is reached: `refused + 1`, `capped_at` set on the
first refusal, which logs once:
`warn  a workspace reached its daily AI limit; its new mail is sorted by rules only until 00:00 UTC  workspace=… cap=…`.
Postgres serializes the conditional upsert on the row, so any number of
workers and processes admit at most `cap` calls a day. **Settling** adds
tokens and failures to the feature row of the admission's day (a call that
crosses midnight stays in the day it was admitted), and is skipped when
there is nothing to add. A call costs at most two short transactions (three
small statements), about a millisecond or two against a 0.3–0.6 s model
call.

No events are published or consumed.

## The measure and the default

**Calls, not tokens.** Tokens are not one unit across backends: laya-serve
reports input tokens of its own encoder (a CPU cost, no output tokens), a
hosted decision model or an instruct model reports billed tokens of its own
tokenizer, and a provider may report none at all, which would leave a token
cap open. A call means the same thing everywhere. A `decide` call's input is
bounded (the envelope is cut at 1,500 characters of body, plus a short fixed
context), so calls bound tokens for it. The ledger records whatever tokens
the provider reports (`prompt_tokens`/`completion_tokens`, or laya-serve's
`input_tokens`/`output_tokens`) so an operator can price a day, but the
limit is on calls. When `complete` gets callers (drafting, the
assistant), its output is not bounded that way, and a token budget joins the
call cap (non-goal here).

**Default 5,000 calls per workspace per day** (4,500 for background work,
500 kept for people), `0` for no cap
(`CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE`; blank is unset):

- Today a message costs one call (the six triage questions go in one
  `decide` call). A busy small business (ten people, 50 inbound emails each
  a day) gets about 500 a day; a bad day double that. 5,000 is five times a
  bad day.
- A first connection runs the model over the last 14 days of mail
  (`DECIDE_BACKFILL_DAYS`): a mailbox getting 150 a day costs about 2,100
  calls once. Two busy mailboxes connected the same day fit.
- A flood stops at 5,000 calls a day, whatever arrives: a known, small daily
  ceiling per workspace instead of an open bill.
- Self-hosters get the same default (a self-hoster may well point `decide`
  at a paid endpoint); one running their own model sets `0`. The issue asks
  for a sane default for self-hosters, so unset is not unlimited.

When features add calls per message, this default is revisited with them.

**Daily = the UTC day.** Workspaces have no time zone of their own; a UTC
day is one moment to explain ("until 00:00 UTC") and one boundary for every
process. The meter reads the time from a `Clock`, so tests move the day
without sleeping.

## Over the cap

The `decide` pass (`decide_message` and `decide_backlog`, one shared
`Pass::run`) gets `OverDailyCap` and returns without storing anything, as it
does when no model is configured or the answers check has not passed. The
message was already stored when it arrived (`apply_changes`): the facts and
the workspace's taught rules have done their part, so it is in the inbox and
searchable, in Primary unless one of them placed it (ADR 0018: the facts
only say no, the model sorts). Only the model's answers, the sorting they
would do and the needs they would raise are missing. Nothing errors and no
job fails or retries.

**Mail skipped over the cap is left as it was stored (option a).**

- (b) *Re-queue for after the reset* would run yesterday's overflow first
  thing the next day. The cap trips mostly on floods; replaying a flood
  burns the next day's cap and starves that day's real mail, every day, and
  even spread over the day it competes with live mail for the same budget.
  Late answers would also move a conversation the person already saw in
  Primary (the model places streams since ADR 0018).
- (c) *A person asks for AI on one message* is the right complement, but it
  is UI (the notice's follow-up); the reserve below is what will let it
  work when the mail has spent its share.
- (a) costs nothing, cannot cause a midnight burst, and loses nothing that a
  person cannot do by hand: the mail is there. The ledger shows how much was
  skipped (`refused`), and a message with no model answers is visible as
  such, so (c) can be built on it.

**A reserve for people (tiered cap).** Background work (the mail's model
pass) stops a tenth of the cap short (`reserve = ceil(cap / 10)`, at most
`cap - 1`: 500 of the default 5,000); calls a person is waiting on
(`Scope::interactive`, today the assistant's compile) may use the whole cap.
One counter, two limits in the same conditional upsert (`$3` is the cap or
the cap less the reserve). A mail flood therefore leaves the assistant 500
answers a day, and only people's own requests can spend them. Past the
whole cap, compile answers 503 `ai_daily_limit_reached` ("Today's AI limit
for this workspace (N calls) has been reached…", four locales). The first
design had no reserve because every call was background work; the
assistant's compile landed on `next` during review and made the reserve
worth its few lines.

## When metering fails

If the ledger cannot be written (a database error), the call is **not
made**: the gateway returns `Error::Metering` and the `decide` pass turns it
into a job retry, so the message is asked again a little later. Rationale:

- It never silently uncaps: a broken ledger cannot become an open bill.
- It does not block mail: storing and rule-sorting happen before, in
  `apply_changes`, which calls no model. Only the model's answers wait.
- It loses nothing: the job retries with backoff (five attempts), and a
  message whose attempts run out is still sorted by its rules, and visible
  in the admin Jobs panel.

If settling fails after the call, the call already counts (admission wrote
it), so only its tokens are lost; the gateway logs a warning and returns the
answer.

## Admin API

`GET /admin/v1/system/ai-usage?day=YYYY-MM-DD&limit=N`,
`system.admin_ai_usage`, `Admin<SystemRead>` (bearer token with
`admin:access`, admin profile with `system.read` or `super`; anyone else
gets 404), audited like every admin request, read in `begin_admin`. Today by
default. Returns the cap (`null` = none), the day, when the day resets, and
for each workspace that used AI that day: calls, refused, whether it hit the
cap, `remaining` (today only) and the calls, failures and tokens by role and
feature; workspaces that hit the cap first, then by calls. Ids and counts
only: no workspace names (they belong to `workspaces`), no content.

## AI role, fallback, undo

- Role: none new. The meter wraps every role (`decide`, `embed`, `complete`,
  `vision`).
- Fallback: over the cap or when metering fails, the caller does what it does
  without a model; for the `decide` pass that is what the facts and taught
  rules decided when the message was stored.
- Undo: nothing is changed by the meter; refusing a call changes no user
  data.

## Alternatives considered

**Where to meter**

1. *At the gateway, with a task-local scope and a meter port (chosen).* One
   seam covers every method and every future caller; unattributed calls are
   refused. Cost: an implicit value (the scope), documented, and a refusal
   that only shows at run time (a test catches it).
2. *An explicit workspace argument on every gateway method and on
   `Strategy::decide`.* Attribution checked by the compiler, but every
   strategy (rules, k-NN, cascade, model), the eval tool and their tests
   change signature, including code being reworked in parallel. The scope
   can become an argument later if the implicit value proves fragile.
3. *A metering wrapper around the model strategy in `conversations`.* Small,
   but it covers `decide` only: `embed` and `chat` callers would skip it. A
   cap that misses a path is no cap.
4. *Count jobs on the `ai` queue.* A job may make zero or several calls, and
   a call outside a job is never counted.

**Where usage lives**

1. *The `system` module (chosen).* It already runs the instance-level AI
   views (the admin System panel and its gateway health), owns a schema, and
   needs no new module.
2. *A new `usage` module.* Cleaner if metering grows to other things
   (storage, sends); more ceremony now for two tables. Can be split out then.
3. *The kernel schema, like rate limits.* The kernel knows no domain, and AI
   usage is the `ai` crate's concern.
4. *In-memory counters per process.* No history for admins, one counter per
   process (the API and the worker would each allow the cap), reset on every
   restart.

**How it is written**

1. *One conditional upsert per call at admission, a settle update only when
   there is something to add (chosen).* Exact across processes, never past
   the cap; about a millisecond per call.
2. *Counters batched in memory and flushed every few seconds.* Fewer writes,
   but each process may overshoot the cap by a batch, and a crash loses the
   batch; the saving is irrelevant next to a model call.

**What is limited**: calls (chosen), tokens (not one unit across backends,
and a provider that reports none would be uncapped) or money (needs a price table per provider
and model, and self-hosted models have no price).

**Over the cap**: leave (chosen), re-queue, or manual; see above.

**Failing metering**: closed with a retry (chosen); open (a broken ledger
would mean no cap at all, silently); closed and dropped (loses the model's
answers for no reason once the database is back).

**From the old Rails app.** The old app had a per-workspace daily call
ceiling for its managed key (`Ai::Budget`, 10,000 a day, self-hosted
exempt). The idea is kept because it wins on the merits: calls are the
honest unit and a daily ceiling is easy to explain. Its mechanics are not
copied: counters in a cache that failed open, an exception class built to
escape rescue clauses, no ledger by feature and no admin view. Here the
ledger is in Postgres with an atomic admission, metering fails closed, the
gateway refuses unattributed calls, and self-hosters get the default too.
