# Design

## 1. Threat model

An attacker has SQL as the runtime role inside workspace W (row-level
security holds them to W) but not the instance secret. After
campbooks-cloud#227 and the rule binding of public #556 they can no longer
send, nor make a rule of their own act. They could still:

- **(8)** insert a `conversations.rule_applications` row for any rule of W
  (a member's real one will do) whose `before` says "the rule reported this
  as spam" (`binned: spam`, `binned_messages: [<the phishing mail>]`), plus
  a `rule_runs` revert run and a `conversations.apply_rule` job. A revert
  run was not gated on any MAC (#556 design §8), so `revert_batch` asked the
  provider, with the server's signature, to take the mail out of spam or
  the trash, unarchive it, take labels off or move it back to a stream;
- **(9)** insert an apply run and its job for a member's real, signed rule.
  The job checked the rule's MAC, which holds, and applied it to the mail
  already in the inbox, which the member had not asked for
  (`apply_to_existing=false`): "trash mail from X" trashed all of X's older
  mail.

After this change both change nothing, at the provider or locally.

## 2. Inventory

### Where a run is created

| Path | Runs in | Signs? |
|---|---|---|
| `conversations.create_rule` (`create_in`, incl. a smart rule from a notice, the assistant, Teach Scout's save) with `apply_to_existing`: apply run | the member's authenticated request, right after it signed the rule it inserted | yes |
| `conversations.confirm_rule` (a proposal's yes) with `apply_to_existing`: apply run | member's request, right after it signed the rule | yes |
| `confirm_rule` of a rule that needs confirming again (`confirm_again`): apply run | member's request, right after it signed the rule | yes |
| `conversations.restore_rule` (Undo) with `apply_to_existing`: apply run | member's request, after it signed the rule again if it verified under the lock | yes, only when the rule is bound; else unsigned, and stopped |
| `conversations.remove_rule?restore=true`: revert run | member's request | yes (§4) |

All five go through `rule_runs::start`, which is the only `INSERT INTO
conversations.rule_runs`. No job, consumer, schedule or startup task
creates a run: `retire` (a rule replaced), `forget_connection`,
`suspended_authors`, `resign_rules` and the job itself only cancel runs or
record their progress. There is no scheduled re-apply.

### Where an application is written

| Path | Writes | Runs in |
|---|---|---|
| `effects::record`, from `apply::apply` (an apply run's batch) | the row, `before` merged with what it held | the job, after the run and the rule verified, for a rule in `rules::acting` |
| `effects::record`, from `arrival::on_arrival` (ingest, `apply_changes`) | same | the ingest transaction, for the rules in `rules::acting` |
| `apply::revert` | `reverted_at` | the job, a verified revert run |
| `exceptions::except` (a member changed back what a rule did) | `excepted_at`, `excepted_by` | member's request |
| `forget_connection` | `before = '{}'` when the inbox is kept read-only | consumer |

### What reads them and acts

| Reader | Acts how | Verified now |
|---|---|---|
| `apply_rule` (any batch) | reads the run (`rule_id`, `kind`, `state`, `cursor`, `counted`) | the run's MAC, at the rule's generation now (§3), before anything else |
| `apply::revert` (a revert run's batch) | asks the provider to undo what `before` says | the application's MAC, under its row lock |
| `apply::apply` | skips a conversation whose application is excepted or still held | not needed: SQL can only make a rule do less |
| `exceptions::except` | finds which rule a member's change takes back, marks it excepted | not needed: local, and only makes a rule leave a conversation alone |
| counts (`find_any`, `attach`, `recent_exceptions`, a revert's total) | display | no |

## 3. What is bound

| MAC | Inputs | Written by | Stored in | Verified by, before |
|---|---|---|---|---|
| A run | workspace, run id, rule id, the rule's `binding_generation`, kind (`apply`/`revert`), the rule's inbox, the requesting member (or none) | `rule_runs::start`, in the member's request | `rule_runs.mac`, `rule_runs.requested_by` | `apply_rule`, every batch, before counting or changing anything |
| An application | workspace, rule id, the rule's inbox, conversation id, run id (or none, for mail arriving), `before` | `effects::record` (`applications::write`), in the effect path, after claiming and locking the row | `rule_applications.mac` | `apply::revert`, under the row lock, before it changes anything |

Labels: `campbooks/conversations.rule-run/v1` and
`campbooks/conversations.rule-application/v1`, one per purpose, under the
kernel's `Binding` (the binding key of campbooks-cloud#227; `0x01 ||
HMAC-SHA256`, constant-time compare). No new key or secret.

**`before` and after.** An application's `before` records both: what the
conversation was (its stream and who placed it) and what the rule did to
it (where it moved it, whether it archived it or marked it read, the
labels it put on, the questions it answered, where it binned which
messages, whether it was archived then). It is encoded as `rules::Before`
serialises (its field order, so a stored copy that parses back encodes the
same; jsonb's key order does not enter) and length-prefixed. A row whose
`before` does not parse as a `Before` never verifies.

**What a run covers.** Runs have no window. An apply run covers the inbox's
mail its rule matches at that generation: the inbox and the generation are
bound, and the conditions with the generation (the rule's own MAC). A
revert run covers what the rule recorded: each application is verified on
its own. The cursor and the counts are progress, not bound: rewriting them
makes a run skip or look again, and looking again never changes a
conversation twice (an application still held is left alone).

**The generation.** A run verifies only at the rule's generation now. Every
signing of the rule adds one (#556), so a rule changed, paused and resumed,
adopted, removed or brought back since a run started stops that run at its
next batch: its content or state may differ from what the member asked to
apply. The member applies it again (restore with `apply_to_existing`, or a
new rule). For a revert run the rule is already removed; nothing but Undo,
which cancels the run anyway, signs it again.

## 4. Signing only where it is safe

**Runs.** `rule_runs::start` signs the run it just inserted, from the
authenticated `Access` (workspace and member) and the rule as stored now in
the same transaction:

- an **apply run** only while the rule is bound: every caller has just
  signed it, or (restore) verified it under its row lock and signed it
  again. An unbound rule's run stays unsigned and the job stops it, as it
  did before;
- a **revert run** whatever the rule's binding: the member asked to put
  back what the rule did, and each application it puts back must verify on
  its own, and only the effect path of a verified rule signs those. Gating
  it on the rule's MAC would add nothing (a member's real rule is enough)
  and would stop a member putting back what a rule did before SQL changed
  it.

SQL controls the rule row's generation and inbox, but not the run id (made
by the request), the member, or the workspace; whatever generation and
inbox it chose are bound as they were, and the job checks them against the
rule as stored when it runs.

**Applications.** `effects::record` runs only where a rule's effects are
carried out, right after the rule verified (`rules::acting` on arrival and
in an apply run, which also verified its run). In this order:

1. **Claim and lock** the row (`applications::claim`): `INSERT … before =
   '{}' ON CONFLICT DO NOTHING RETURNING`; a row returned is ours. Otherwise
   `SELECT … FOR UPDATE`, which waits for a transaction still inserting the
   row and returns it as it committed. A bare `SELECT … FOR UPDATE` would
   lock nothing while the row does not exist yet: an attacker's uncommitted
   INSERT with a forged `before` would be invisible to it, and the upsert's
   `ON CONFLICT DO UPDATE` would then merge into it once it committed
   (the race the review of this PR found).
2. **Verify** what the locked row recorded. A row that does not verify
   (planted, edited, or written before this change) is dropped (a WARN with
   ids): nothing of it is kept.
3. **Merge in Rust** (`applications::merge`) the verified `Before` (or none)
   with what this path did: the first stream and bin, the last place it
   moved it, everything it did either time.
4. **Write `before`, run and MAC in one statement** (`applications::write`),
   overwriting the row whole: the value signed is the one computed in Rust,
   never one SQL produced.

**No signing oracle.** No job signs a run, and no job creates one. The
only job that signs an application is `apply_rule`'s apply batch, and only
for a rule and run that verified, over its own effects; arrival does the
same in ingest. A planted job or run only reaches code that verifies. There
is no re-sign of rows kept at the deploy (§6).

## 5. On a failed check

- `apply_rule`: `WARN a rule run is not bound to the member who asked for
  it, or its rule changed since; it is stopped` (workspace, rule, run); the
  run ends `cancelled`, the job completes, no retry. For an apply run the
  rule's own check (#556) follows.
- `apply::revert`: `WARN an application of a taught rule is not bound to
  it; it is not put back` (workspace, rule, conversation). The row is left
  as it is and the batch goes on with the others; the run's `done` counts
  it, `changed` does not.
- `effects::record` (`claim`): `WARN an application of a taught rule is not
  bound to it; what it recorded is dropped` (workspace, rule, conversation);
  ingest and the batch go on.

No WARN carries a `before`, an address, a provider id or the tag. A row
that does not decode is never an error ingest or a job fails on.

## 6. Rows that exist at the deploy

Re-signing old rows would sign whatever SQL wrote (§4), so none are:

- **In-flight runs** have no MAC: each stops at its next batch with the WARN
  above and ends `cancelled`. The member applies the rule again (restore it
  with `apply_to_existing` within Undo's window, or teach it again). Runs
  are short (100 conversations a batch) and few.
- **Old applications** have no MAC. Putting them back would need SQL's
  word for what they say, so it is refused: `conversations.remove_rule`
  with `restore=true` answers **409 `undo_unavailable`** for a rule that
  still holds an application with no MAC, before anything changes. Removing
  it keeping its changes (`restore=false`) still works. The client shows its
  usual "the rule did not change" message for a failed removal; a screen
  offering to keep the changes is later work. A held application with a
  wrong MAC (not NULL) is planted or edited, not old: it is skipped by the
  revert with the WARN above, while the rest go back.
- A conversation an old rule changes **again** after the deploy (new mail in
  it) drops what its old application recorded (WARN above) and keeps, for
  undo, only what the rule does from then on.

Planted rows with no MAC also make `restore=true` answer 409: SQL can
refuse a member's undo (they still remove the rule), never act through it.

**Other upsert-then-sign paths.** Looked for in this change and in the
rule binding of #556: `rules::binding::sign` signs a rule its caller holds
locked (the request's own `INSERT` of a new id, or `FOR UPDATE` taken
before reading it), never through an upsert; `resign_rules` locks with
`FOR UPDATE SKIP LOCKED` and the definer function compares SQL with SQL; a
run is signed after a plain `INSERT` of an id the request made (a planted
row with that id would make the insert fail, never merge). The only
upsert-then-sign was `effects::record`, fixed as above. The other upserts
of the module (`store_decision`, `link_label`) sign nothing.

## 7. What is still open

- **Replay.** A run or an application row, MAC included, can be restored by
  SQL exactly as a member's request or the effect path left it (a run's
  state set back to `running`, an application's `reverted_at` or
  `excepted_at` cleared). A replayed apply run at the same generation acts
  as the member asked then (every conversation it already changed is left
  alone, excepted ones too); a replayed revert puts back again only what a
  verified application says, where the conversation still holds it. Same
  limit, same fix as campbooks-cloud#227 §8 (state outside the database).
- **`applied_at`, `reverted_at`, `excepted_at`, `excepted_by`** are not
  bound: SQL can make a rule leave a conversation alone, or offer a
  verified application to the next undo again (as above).
- **What the effect path reads.** `before.stream` comes from the
  conversation row, `binned_messages` from the messages' provider ids, and a
  label's provider key from the labels table, all SQL-writable: rewriting
  them before a rule acts changes what its undo later names. Item 4 of
  campbooks-cloud#242 is the same class.
- **The requesting member** is bound but not checked again when the run
  acts; the rule's author is (`effective_state`).
- **`forget_connection`** clears a detached inbox's applications'
  `before`; nothing is put back there, and if the inbox were attached again
  those rows would no longer verify and be skipped.

## 8. Migration

`20261022010000_conversations__rule_run_bindings.sql`, the first free slot
after `next`'s newest (`20261022000000`); no open PR holds a migration. A
10 s lock timeout; three nullable columns, catalog-only. No grant, policy,
table or function changes.

### Rollback SQL

sqlx refuses to start with an applied migration it does not know. Before
redeploying an image older than this change, run once as the migration
login:

```sql
-- Roll back campbooks-cloud#242 items 8-9 BEFORE redeploying an older image.
BEGIN;
SET LOCAL lock_timeout = '10s';
ALTER TABLE conversations.rule_applications DROP COLUMN mac;
ALTER TABLE conversations.rule_runs DROP COLUMN mac, DROP COLUMN requested_by;
DELETE FROM _sqlx_migrations WHERE version = 20261022010000;
COMMIT;
```

The older image then runs and puts back runs and applications unchecked.

## Prod risks

- **Runs going at the deploy stop** at their next batch (a WARN each): mail
  not reached yet by an apply run is not changed, and a revert run leaves
  the rest of the mail where the rule put it. Nothing retries them.
- **Undo of older rules is refused** (`undo_unavailable`) until every
  application they hold from before the deploy is gone; the member removes
  them keeping their changes, and puts mail back by hand.
- **A run stops when its rule is signed again**: a member who pauses and
  resumes, reshapes or adopts a rule while its run is going sees the run
  end `cancelled`.
- **Ingest does three more small queries** per rule application (read,
  merge, sign), inside the same transaction.
- **The MACs depend on the instance secret**, as sends and rules do: an API
  and a worker with different secrets would refuse each other's runs and
  applications. Without `CAMPBOOKS_SECRET_KEY` no rule acts at all (#556).
