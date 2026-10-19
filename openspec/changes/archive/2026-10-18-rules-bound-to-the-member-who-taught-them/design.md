# Design

## 1. Threat model

An attacker has SQL as the runtime role inside workspace W (row-level
security holds them to W) but not the instance secret. Before this change
they could insert a row of `conversations.rules` (any manager as
`taught_by`, conditions matching everything, `trash`), or change an existing
rule's conditions, actions, inbox, author or state. The next inbound mail ran
it in `rules::on_arrival` during ingest, with no job to plant; a planted
`conversations.rule_runs` row and `conversations.apply_rule` job ran it over
the mail already there. Its changes were recorded and signed as the server's
own (`effects::request` → `conversation::request`, campbooks-cloud#227), so
Gmail or Zoho trashed, labelled, archived or reported them as spam. A planted
"Always show images" rule also made a sender's remote images load.

After this change a rule acts only when its MAC verifies, and only a member's
request signs one.

## 2. Inventory

### Paths that create or change a rule

| Path | Runs in | Signs? |
|---|---|---|
| `conversations.create_rule` (`features::rules::create_in`), incl. a smart rule from "Scout noticed" (`notice_id`), "Always show images" (#549, a `show_images` rule saved through it), Teach Scout's sentence (`assistant.compile_instruction` saves nothing: the member previews and saves through `create_rule`) and the assistant (it invokes operations with the member's principal) | the member's authenticated request | yes, when created active; a proposal is not |
| `conversations.confirm_rule` (a proposal, or a kept rule that needs confirming again) | member's request | yes |
| `conversations.update_rule` (reshape, pause, resume, adopt) | member's request | yes, after verifying the rule under its row lock; an unbound rule may only be paused, unsigned |
| `conversations.remove_rule` | member's request | yes (state `removed`), only if it verified under the lock |
| `conversations.restore_rule` (Undo) | member's request | yes, only if it verified (as removed) under the lock |
| `replace` → `retire` (a new active rule retires an older one of the same shape) | member's request | yes (state `removed`), only if it verified |
| `conversations.decline_rule` | member's request | no (a proposal; it stays unsigned) |
| `forget_connection` (clears `origin_text`), `rules::effects::record` / `apply::revert` (`applied_count`, `last_applied_at`) | jobs and consumers | no; none of these fields is bound |
| `suggest_rules`, `rule_notices` (Scout noticed), `fill_templates` | requests and jobs | no; they store suggestions and templates, never a rule |
| `suspended_authors` | consumer | no; it reads rules and announces them (a suspension pauses a rule through `effective_state`, not its row) |
| `taught.rs` (`campbooks_ai::decide::taught`) | the engine | no; it only evaluates |
| Migrations | none seeds a rule | `…_rule_bindings` snapshots the kept ones (§6) |
| `conversations.resign_rules` | job | only a rule the snapshot holds whose stored fields still hash to the snapshot's (§6) |

### Paths that run a rule's effects

| Path | Inputs SQL controls | Verified where |
|---|---|---|
| `rules::on_arrival` (ingest, `apply_changes`) | the rules table | `rules::acting` (the rules the engine evaluates for the batch) |
| `conversations.apply_rule`, apply run | the run row, the job payload, the rule | the job, before counting or changing anything; the competing rules through `rules::acting` |
| `conversations.apply_rule`, revert run | the run row, `rule_applications.before` | not gated on the rule's MAC (§8) |
| "Always show images" (`rules::images_trusted`, read by `get_message_body`) | the rules table | `rules::acting` |
| Previews (`compute_preview`), the rules list and summaries | the rules table | not gated: they change nothing; the list carries `needs_confirmation` |

## 3. What is bound

| MAC | Inputs | Written by | Stored in | Verified by, before |
|---|---|---|---|---|
| A taught rule | workspace, rule id, binding generation, inbox, `taught_by` (or none), stored state, precedence (`confirmed_at`, else `created_at`, in microseconds), and the content hash | the member's request (`rules::binding::sign`), the one-off re-sign (§6) | `rules.mac`, `rules.binding_generation` | `rules::acting` (arrival, runs, images); `apply_rule` before an apply run acts; `update_rule`, `remove_rule`, `restore_rule`, `retire` before they sign it again |

The **content hash** is SHA-256 of a `Fields`
(`campbooks/conversations.rule-content/v1`) over the conditions (a count,
then each condition's typed value serialised) and the actions (likewise), in
stored order: every field that decides what a rule matches (sender, domain,
list, subject template or words, header, label, stream, failed checks) and
what it does (stream, archive, label, read, the three answers, trash, spam,
paid, showing images). The specificity is computed from the conditions, so
it is bound with them; the precedence among equally specific rules is the
confirmation time, bound on its own.

The tag is the kernel's (`0x01 || HMAC-SHA256`, constant-time compare) under
the binding key of campbooks-cloud#227, domain-separated by its own context
label `campbooks/conversations.rule/v1`. No new key or secret.

**The state** is bound so that a state a member left holds: a rule Sam paused
cannot be made to act again by setting `state = 'active'`, nor a removed one
brought back, without the MAC the member's own request left for that state.
So pausing, resuming, removing and bringing back each sign again.

**The generation** (`binding_generation`) grows at every signing. SQL can
write it too, so it does not stop the replay of §8; it makes every signing a
distinct input.

## 4. Signing only where a member asks

`rules::binding::sign` signs the rule as stored, at the next generation. Its
callers hold the rule's row lock (the request's own `INSERT`/`UPDATE`, or
`FOR UPDATE` taken before reading it) and vouch that every bound field is
the member's: set by the request itself, or verified under that lock first.
So:

- `create_rule` signs the row it inserted (an active rule); a proposal waits
  for a yes.
- `confirm_rule` signs the proposal the member says yes to, read under the
  lock (the member saw it in the list). A kept rule that does not verify is
  confirmed again the same way: anyone who may teach in its inbox, as for a
  proposal. It keeps its state, but its author becomes the confirming
  member: the stored `taught_by` is SQL's to choose, and a NULL one would
  skip every author check. A run starts only when asked and it is active.
- `update_rule` now takes the rule's runs' and row locks first (the order
  `confirm_rule` keeps), reads it again and verifies it. A bound rule is
  signed again after the change. An unbound one may only be paused (unsigned,
  so it stays waiting); a reshape (even of the actions alone, which keeps the
  conditions and author SQL wrote), a resume or an adopt answers 409
  `rule_needs_confirmation`.
- `remove_rule`, `restore_rule` and `retire` verify the rule under its lock
  and sign it again in its new state only if it verified.

A path that turns AI output into a rule does so only at the member's yes: no
job stores an active rule.

Without a key `sign` fails the request (500, a WARN with ids), as
`send_draft` does: a rule saved active would never act.

## 5. On a failed check

- `rules::acting`: `WARN a taught rule is not bound to the member who taught
  it; it does not act` (workspace, rule). The rule is left out of what the
  engine sees: it places, flags, labels, answers and shows images for
  nothing, and asks the provider for nothing. Every other rule still acts.
- `apply_rule`, apply run: `WARN a taught rule is not bound to the member who
  taught it; its run is stopped` (workspace, rule, run); the run ends
  `cancelled`, the job completes (no retry).
- `resign_rules`: `WARN a taught rule changed since rules were bound; it
  waits for a member to confirm it` (workspace, rule).
- A row that does not decode (conditions or actions that are not a rule's) is
  treated as no rule, in every list and lookup, with `WARN a taught rule does
  not decode; it is left out` (rule): it cannot fail ingest or a request, and
  its apply run ends `cancelled` with no retry.

No WARN carries a condition, an action, an address or the tag.

The rule shows `needs_confirmation: true` (kept, active or paused, and not
bound) in `list_rules` and `get_rule`, with its stored state. A screen that
asks for the yes is a later change; the operation is there
(`confirm_rule`).

## 6. Rules kept at the deploy

Re-signing every rule at startup would sign whatever SQL wrote: the hole this
change closes. Instead:

1. The migration computes, in SQL, a SHA-256 of each kept rule's (active or
   paused) bound fields, as text no session setting changes:
   `jsonb_build_array(workspace_id, inbox_id, taught_by, state, conditions,
   actions, to_char(coalesce(confirmed_at, created_at) AT TIME ZONE 'UTC',
   'YYYY-MM-DD"T"HH24:MI:SS.US'))::text` (uuids and jsonb print one way; the
   time zone, date style and float digits do not enter). It stores the rule
   id, workspace and hash in `conversations.rule_resigns`.
2. `rule_resigns` has row-level security (the workspace; the maintenance
   scope may read), the runtime role may only `SELECT` it, and its rows can
   only be removed by `conversations.take_rule_resign(uuid)`: `SECURITY
   DEFINER`, owned by `campbooks_definer` (#529), `search_path = pg_catalog,
   pg_temp`, every name qualified, granted to the runtime role only. It
   deletes the rule's row (in the transaction's workspace) and returns
   whether the rule, as stored now, is kept and hashes to the snapshot's
   value, with the same expression.
3. `conversations.dispatch_rule_resigns` (every minute, the maintenance
   scope, `architecture.toml`) enqueues `conversations.resign_rules` in each
   workspace that still has rows. That job, for each row, locks the rule
   (`FOR UPDATE SKIP LOCKED`: a member's request holding it wins; the next
   tick retries), calls the function, and signs the rule as stored only on
   `true`.

**Why it is not a signing oracle.** What the job signs is exactly what the
migration hashed: SQL cannot add a row (no `INSERT` grant), change a hash (no
`UPDATE`), or make a changed rule hash the same. A planted job, or a call of
the function, only consumes rows that are already there: at worst a rule
kept at the deploy waits for a member's yes. The comparison is SQL against
SQL, so it does not depend on the Rust encoding matching a SQL one.

A rule that was already planted with SQL before the deploy is signed too:
nothing in the database tells it from the owner's. That is the trust the
upgrade has to start from, once.

Without an instance secret the job takes nothing, so the rows wait.

## 7. Effects covered

Every effect a rule has goes through `rules::acting`, so all of them are
covered: placing in a stream, archive, mark read, labels, trash, spam (each
signed as a change request for the provider), the answers it gives
(`needs_reply`, `asks_action`, `surface`, paid; local, they decide what Home
and the inbox show), and showing a sender's remote images (local, read when a
message opens: a planted trusted sender would make tracking-free images
load, which reveals that the mail was opened). Previews and the rules list
read every rule but change nothing.

## 8. What is still open

- **Replay of a state a member left.** A rule's row, MAC included, can be
  restored by SQL exactly as a member's request left it. Someone who read a
  rule's MAC while it was active can bring it back after its member removed
  it. Same limit, same fix as campbooks-cloud#227 §8 (state outside the
  database).
- **`removed_from`** (the state Undo brings a rule back to) is not bound: SQL
  can make Undo bring back as active a rule that was paused when removed; the
  member asked to bring it back, and its content is theirs.
- **Revert runs** are not gated on the rule's MAC: they put back what the
  rule's own `rule_applications.before` rows recorded (unarchive, out of the
  trash or spam, unread, labels off, back to its stream, archive again what a
  spam rule took from the archive), and those rows, like the run rows, are
  not bound. A planted revert run can therefore put back what planted
  `before` rows say. Planted revert or apply runs (MACs on `rule_runs` and
  `rule_applications`) are tracked in campbooks-cloud#242. Gating it on the rule's MAC would not help (a member's
  own rule's id is enough) and would stop a member putting back what a rule
  did before it was changed with SQL. Binding the applications is a
  follow-up.
- **`messages.provider_message_id`**, and the label and stream rows a change
  names, are read from SQL-writable rows when a change is requested (for
  members' own actions too): rewriting them moves a member's or a rule's
  change to another message of the same mailbox. Item 4 of
  campbooks-cloud#242 is the same class.
- **Membership.** A rule acts while its author is a sender or manager of the
  inbox (`inbox_members`, SQL-writable): SQL can keep a removed author's
  rule acting. The rule's content is still the one its member taught.
- **The confirmation a member gives** is to the rule as the list showed it;
  SQL could change it between the list and the click (item 2 of
  campbooks-cloud#242 is the same for drafts).

## 9. Migration

`20261022000000_conversations__rule_bindings.sql`, the first free slot after
`next`'s newest (`20261021230100`); no open PR holds a later one. A 10 s lock
timeout; the two new columns are catalog-only.

### Rollback SQL

sqlx refuses to start with an applied migration it does not know. Before
redeploying an image older than this change, run once as the migration
login:

```sql
-- Roll back campbooks-cloud#242 part 1 BEFORE redeploying an older image.
BEGIN;
SET LOCAL lock_timeout = '10s';
DROP FUNCTION conversations.take_rule_resign(uuid);
DROP TABLE conversations.rule_resigns;
REVOKE SELECT ON conversations.rules FROM campbooks_definer;
REVOKE USAGE ON SCHEMA conversations FROM campbooks_definer;
ALTER TABLE conversations.rules DROP COLUMN mac, DROP COLUMN binding_generation;
DELETE FROM _sqlx_migrations WHERE version = 20261022000000;
COMMIT;
```

The older image then runs every rule again, bound or not.

## Prod risks

- **Rules stop for a minute at the deploy.** Until `resign_rules` has run
  (its dispatcher ticks every minute), no kept rule acts: mail arriving in
  that window is not sorted, archived or labelled by a rule, and is not
  looked at again later. WARN `a taught rule is not bound…` lines for each
  rule in that window are expected.
- **If the re-sign cannot run** (the instance secret missing in the worker,
  `campbooks_definer` without its grants, the maintenance queue stuck), every
  rule stays unbound and stops acting: no rule sorts or archives mail, and
  each shows `needs_confirmation`. Inbound sync, sends and members' own
  actions are unaffected. Watch for `taught rules are not bound: there is no
  key…` and repeated `a taught rule is not bound…` WARNs after the first
  minutes, and for rows left in `conversations.rule_resigns`.
- **A rule's MAC depends on the instance secret**, as sends do: an API and a
  worker with different secrets would refuse each other's rules.
- **`update_rule` takes one more lock** (the rule's runs, then its row,
  before the rules it replaces), the order `confirm_rule` already keeps.
- **An instance without `CAMPBOOKS_SECRET_KEY`** can no longer save an active
  rule (500), as it cannot send.
