# Design

Prototype (desktop and 375px, demo data):
https://claude.ai/artifact/2y7z5yvKsMpKC6nVTsPrw7

## Owner directions (2026-10-14)

- "The purpose of actions of email is that visor does them for me, at least
  after the first. I have 1000 emails that look the same and I said one
  should be moved to a certain place, all of them should be."
- "Rules should be translated into a smart rules system that become
  deterministic." Teach-once is the first producer of one shared rules system.
  Every teaching (an action, an onboarding answer, a free-text instruction
  from #121, confirmed corrections) compiles into deterministic
  condition → action rules. Rules run before any model, always give the same
  result for the same mail, are explainable, and are listed and editable in
  Scout's memory. The rule is proposed with a match preview and the member
  confirms; ambiguity is asked about, not guessed. Later, suggestions the
  model or k-NN keep getting confirmed can be proposed as new rules.
- "Don't wait for my approval to improve the AI system": the open questions
  below carry a recommended default, and the implementation follows the
  defaults in this same change.

## Context

What exists on `next` (read 2026-10-14):

- `conversations.move_to_stream` sets `stream_source = 'member'` and writes a
  row to `conversations.corrections` (question `stream`, from, to, the source
  it overrode, the decisions on record). Nothing reads corrections. Later mail
  never re-sorts a conversation (`sort_first_inbound` only updates
  `stream_source = 'default'`).
- The decide pass: `EmailSignals` (the built-in rules, `Source::Rules`) runs
  in `apply_changes` for each new inbound message; `Policy::baseline()` lets
  rules act on `needs_reply`, `asks_action`, `has_deadline` and `urgency` and
  only records `stream` and `suspicious`; the model pass
  (`conversations.decide_message`) stores answers and never re-sorts.
  `campbooks_ai::decide::knn::Knn` and `Cascade` exist but run only in the
  eval harness (`cargo xtask eval decide`).
- Conversation state is **shared by the inbox's members**: read, starred,
  archived, labels (per inbox), stream and the need. The only per-member
  state is attention's Later (`attention.snoozes`).
- Write-back: every tidy operation publishes `conversations.changes_requested`
  (v1, one conversation, items per provider message); `connectors` turns it
  into `outbound_changes` and pushes them in its sync run. Gmail: read, star,
  archive and labels as label edits, streams as categories (Receipts has no
  equivalent), `batchModify` for 11 or more messages. Zoho: read, flag,
  archive; labels and streams are not mirrored. The provider budget is a
  token bucket per provider (`connectors.provider_budgets`).
- Messages store `sender_address` (lower-cased), `subject`, and a headers
  subset (`list_id`, `list_unsubscribe`, `precedence`, `auto_submitted`).
  No sender-domain column, no subject normalisation, no index on either.
- Jobs: `kernel.jobs` with unique keys while pending, ordering keys and
  per-workspace fairness; long work re-enqueues itself page by page with a
  cursor kept in a table (`search.backfill`).
- Undo is an operation named in `x-campbooks.undo` (ADR 0013); the client's
  `UndoProvider` keeps the stack (Z).
- Access: forced RLS per workspace; inbox roles `reader < sender < manager`.
  ADR 0017 (PR #479, not merged yet) adds a resource catalogue and one
  `kernel::authz::authorize` check; this change declares against it and
  falls back to the module's existing `lock_conversation` / role helpers
  until it lands.
- No rule, filter, memory or activity concept exists in code. The domain map
  reserves **Rule** for `automation` (M2) and defines **Memory** in
  `assistant` as "what the member taught Campbooks, shown as sentences backed
  by real rules and settings".

## Terms

- **Taught rule**: a deterministic condition → action pair a member taught
  Campbooks for one inbox. UI copy says "rule"; code and specs say *taught
  rule* (`conversations.rules`, decision source `taught`, stream source
  `taught`) because the code already calls the built-in email signals
  "rules" (`Source::Rules`, `stream_source = 'rules'`) and the domain map
  keeps **Rule** for automation. _Avoid_: habit, filter, inbox rule.
- **Likeness**: the conditions that say which mail a rule covers, rendered as
  plain words ("from notifications@github.com").
- **Teaching**: the act that produced a rule (an action, an onboarding
  answer, an instruction, corrections, a suggestion): its **origin**.
- **Run**: one pass of a rule over existing mail (apply or revert).
- **Application**: one conversation a rule changed, with what it was before.
- **Exception**: a conversation a member changed back after a rule acted on
  it; the rule leaves it alone from then on.

`docs/domain-map.md` gains these under conversations' language.

## Decision 1: where the rules system lives

| | **a. `conversations` owns rules; engine in `campbooks-ai`** | b. A new `rules` module behind a kernel port | c. Build `automation` (#44) now; teach-once creates automation rules | d. Feed corrections to k-NN only (#83) | e. Provider filters (Gmail filters, Zoho rules) |
| --- | --- | --- | --- | --- | --- |
| Runs before any model, same answer every time | yes, inside the ingest transaction | yes, through a synchronous port call per message | no: rules react to `message_received` after the conversation exists, so mail lands in Primary first and moves (two write-backs) | no: probabilistic, abstains | at the provider only |
| Existing mail | set-based runs over the module's own tables and indexes | needs a second copy of mail facts or a query port | per-message operation invocations (1,000 calls, 1,000 events) | no | no (filters apply to new mail) |
| Explainable as a sentence | yes, from typed conditions | yes | yes | no ("similar to 5 you corrected") | partly, not in Campbooks |
| New machinery | a table set, a pure engine, one job | a module, a port, its schema, an ADR | a whole module: triggers, conditions, runs, loop protection | none | a scope (`gmail.settings.basic`), per-provider code, IMAP has none |
| Sources of truth | one (conversations) | two (rules + conversations), kept in sync by events | one, but far from the data it reads | corrections | two (provider + Campbooks); streams are not a provider concept |
| Self-host weight | none | none | none | an `embed` role or the lexical floor | none |

**Chosen: a.** Every condition reads conversations' own data (sender, list,
subject, headers, labels, stream) and every action is a conversations
operation or a decide answer. Keeping storage, matching and application in
the module that owns the data is what makes "runs first, same answer every
time" cheap: the rule stage runs in `apply_changes` in the same transaction
that creates the conversation, and a bulk run is a keyset scan over an index
of the module's own tables. The matcher itself is pure and lives in
`campbooks-ai` as `decide::taught`, beside `EmailSignals` and `Knn`, so it is
also a `Strategy` the eval harness and #83's `Cascade` can put first
(taught > signals > knn > model).

The other producers fit without new seams: #121 compiles language and
onboarding answers into the same conditions and actions and calls
`conversations.preview_rule` / `create_rule`; corrections and model
suggestions become *proposed* rules. When automation (#44) arrives it builds
on this model (a taught rule is automation's narrowest case: trigger "mail
arrives in this inbox", conditions on its facts, conversations actions) and
lists taught rules as its own; rules about other records (documents, money)
use the same engine over their own facts, stored by the module that owns
them. Option b is the path if rules ever need to span modules in one
transaction; nothing needs that today.

## Decision 2: what "looks the same" means

Deterministic first. Each likeness is a condition set, explained in words
the member recognises, with its precision risk:

| Likeness | Conditions | Said as | Precision risk | Guard |
| --- | --- | --- | --- | --- |
| **Sender** | `sender_address is` | "from notifications@github.com" | A sender that sends several kinds of mail (shipping and password resets from one no-reply); a *person*, whose next message may matter | Default only for machine senders the inbox never wrote to (below); hiding actions never offered for correspondents |
| **List** | `list_id is` | "the acme/web list" | Lowest: a List-Id names one list | Only when the message carries one |
| **Sender + subject words** | `sender_address is` + `subject_contains` | "from notifications@github.com about "pull request"" | Words that also appear in other kinds of mail | Words come from the template's literal words the acted message shares with ≥ 3 lookalikes |
| **Sender + subject template** | `sender_address is` + `subject_template is` | "the "Your order # has shipped" emails from shop.example" | Misses variants with other wording (low recall, high precision) | Template needs ≥ 2 literal words |
| **Domain + subject template** | `sender_domain is` + `subject_template is` | "the "Your order # has shipped" emails from shop.example" | A domain hosting both people and machines | Never a free-mail domain (gmail.com, outlook.com, …); always with a template; offered when the sender's local part varies per message (`order-1234@`) |
| Similar (later) | k-NN neighbours (#83) | "like 5 you moved" | Opaque, abstains, needs evals | Not in this change; proposed as a rule only after repeated confirmations |

**Subject template**: lower-case, strip `Re:`/`Fwd:` prefixes (four
locales), replace dates and digit runs with `#`, tokens of 6+ characters
mixing letters and digits (ids, codes) with `*`, collapse spaces, keep ≤ 120
characters; at least two literal words or no template. Stored per message
(`messages.subject_template`) at ingest and backfilled, indexed with the
sender.

**Machine sender**: the email signals already tell a service from a person
(bulk headers, `no-reply`/`notifications` local parts, `Auto-Submitted`).
A sender counts as one when its message carries a bulk or auto-submitted
header or the signals' service reason, and the inbox has never sent mail to
that address (a *correspondent* is anyone the inbox wrote to).

**Which likeness is offered first (the default):**

1. Sender, when the sender is a machine sender (the owner's 1,000
   notifications case);
2. else the List, when there is one;
3. else Sender + template (or Domain + template when the address varies);
4. a person who is a correspondent gets no default: the offer asks "Which
   mail?" with the choices unselected, and hiding actions (archive, read,
   Not needed) are not offered at all.

Up to three likenesses are shown, broad to narrow, each with its count, so
the precision trade-off is the member's choice in plain words. Counts are
conversations in the inbox whose newest inbound message matches, that the
rule would change (not already so), excluding suspicious mail and
conversations a member placed by hand.

## Decision 3: the rule model

```
Rule      { inbox, teaching_id, conditions[1..=4] (AND), actions[1..=4],
            origin: action|onboarding|instruction|correction|suggestion,
            origin_text?, state: proposed|active|paused|removed,
            taught_by, from_conversation?, confirmed_at, applied_count,
            last_applied_at }
Condition { field, op, value }
  sender_address is · sender_domain is · list_id is · subject_template is
  subject_contains (whole words, case- and accent-insensitive)
  header is|present (one of the stored headers) · label has · stream is
Action    move_to_stream{stream} · archive · add_label{label} · mark_read
          needs_reply{bool} · asks_action{bool} · surface{always|never}
```

- **Slots.** Each action fills a slot: `stream`, `archive`, `read`,
  `needs_reply`, `asks_action`, `surface`; `add_label` is additive. A rule
  has at most one action per slot.
- **Determinism.** For one message and one rule set the outcome is fixed:
  per slot, the matching active rule with the highest specificity wins
  (template, List-Id or header 3; subject words or sender address 2; domain,
  label or stream 1; summed over its conditions), then the latest
  `confirmed_at`, then the id. Different slots combine; labels add up.
- **Where it runs.** In `apply_changes`, for each new inbound message, after
  the built-in signals and before placement: taught answers override the
  signals per question (stream, needs_reply, asks_action) and are stored in
  `decisions` with source `taught`, the rule id as the reason, and
  `acted = true` (`Policy` treats `taught` as always acting: a member
  confirmed it). Archive, read and label actions apply to the new message.
  The model pass never overrides a taught answer. Stream precedence becomes
  `default < rules | model < taught < member`.
- **Surface.** `never`: the derived need is not raised (an open one clears
  with cause `rule`). `always`: no rule may archive, mark read or move the
  conversation out of Primary, and a need is raised as Review ("You asked to
  see mail from …") when nothing else applies.
- **Suspicious mail.** When the message has a true `suspicious` answer (any
  source) or an open Review need, rules never archive, mark read, move out of
  Primary, answer a need false, or say `surface: never`. The warning always
  shows. A suspicious conversation never starts an offer.
- **Proposed rules** (from #121, from corrections, from suggestions) do
  nothing until confirmed (`confirm_rule`); they are listed under "Waiting
  for you" with their preview.

## Decision 4: which actions teach

| Member action | Teaches | Why |
| --- | --- | --- |
| Move to a stream (M, 1–4) | `move_to_stream` | The owner's case. |
| Done in the inbox (E, archive) | `archive` | "Mail like this never needs to sit in my inbox." A Pay need stays open when archived (existing rule), so bills are not lost. |
| Add a label | `add_label` | Labels are per inbox and shared. |
| Mark read (U) on an unread conversation | `mark_read` | Only the explicit key, never the read-on-open. Also offered as "Also mark them read" with the other actions. |
| Not needed (X) on a need | `needs_reply` or `asks_action` false (the question that raised it); `surface: never` for a Pay need | Teaches "these never need me". Not on a Review need ("It's safe" is about one message). |
| Done on a need, Later, star, reply, unarchive, remove label, mark unread | nothing | Done is "I did this one"; Later is personal and about time; the inverses are exceptions (Decision 7). |

The offer appears when the default likeness covers at least **2 other**
conversations, or the message has a List-Id (new mail is likely). "Just this
one" (Esc) dismisses it for that likeness and action for 30 days; a second
dismissal stops it for good (`conversations.dismiss_rule_suggestion`,
per member).

## Decision 5: first time and after

- **First time** (prototype moment 1). The action happens at once with its
  usual toast and Undo. The client then calls `conversations.suggest_rules`
  for that conversation and action. When there is a suggestion, an offer
  card docks above the toast (a sheet on phones): Visor, the question
  ("Move all 1,000 like this to Updates, and new ones too?"), the likeness
  choices with counts, five real subjects with dates, "Also mark them read"
  where it applies, **Move all 1,000 (Y)** and **Just this one (Esc)**. In a
  shared inbox it says "For everyone in info@". The card never blocks the
  list: J, K and E still work and dismiss it.
- **Accept** calls `conversations.create_rule` with `apply_to_existing`; the
  card becomes the run's progress (moment 2) from `conversations.rule_changed`
  on the change feed: "Moving 1,000 to Updates · 340 of 1,000 done". The
  member can keep working; the run continues with the app closed. At the
  end: "Moved 998 to Updates. New ones will go there too. Left 2 alone: they
  look suspicious", with **Undo all (Z)** and a link to Scout's memory.
- **After** (moment 3): new matching mail is handled on arrival. Its row
  shows "Visor moved this · like 1,000 others from notifications@github.com"
  in the slot suggestions use; the reader's note says "Visor moved this to
  Updates, like 1,012 others from notifications@github.com" with **Move back
  (M)** and **Stop doing this**, and when it was taught. A conversation
  summary carries `rule: { id, action, applied_count }` when a rule placed or
  changed it.
- **Scout's memory** (moment 4): Settings → Scout's memory lists rules per
  inbox as sentences built on the client from conditions and actions in the
  four locales ("Mail from notifications@github.com about "pull request" →
  Updates · 1,000 so far"), with origin, who taught it, last applied,
  exceptions, Edit and Remove. Remove confirms in place and offers "Also put
  back the 1,012 it moved". Proposed rules sit under "Waiting for you" with
  **Keep this rule (Y)** and **No (Esc)**.

## Decision 6: shared inboxes

| | **a. A rule is the inbox's, for everyone** | b. A rule acts only for its author | c. Only managers teach |
| --- | --- | --- | --- |
| Fits the data | yes: stream, archive, read, labels and needs are shared already | no: there is no per-member stream or archive | yes |
| Surprise for others | a second member sees mail move; mitigated by "For everyone" and the row line naming the rule | none, but nothing to apply | none |
| Friction | none | n/a | senders can't teach |

**Chosen: a.** Teaching takes the `sender` role (the same as tidying); a
`reader` is offered nothing (403 `inbox_read_only`). The offer and the
memory page say "For everyone in info@". Editing and removing take the
author (while they still hold `sender`) or an inbox `manager`; any sender
can make an exception on one conversation. When the author loses `sender`
on the inbox (left, demoted), their rules pause (`paused`); managers see
them with Adopt and Remove. Personal state stays personal: Later is never a
rule action, and a member's Later on a conversation is untouched by rules.

ADR 0017 declaration (resource type `conversations.rule`, container: the
inbox): `read` to the container's `reader`; `create` to `sender`; `write`
and `delete` to the owner while they hold `sender`, and to `manager`;
client tokens get `read` only. Workspace owners and admins get nothing in an
inbox they are not a member of.

## Decision 7: bulk mechanics

| | **a. Set-based runs in batches, in the module** | b. Invoke the per-conversation operation 1,000 times | c. One giant transaction |
| --- | --- | --- | --- |
| Progress, resume | a run row with a keyset cursor; a crash resumes from it | one job each; progress is a count of jobs | none; all or nothing |
| Events | `conversation_changed` per conversation, published per batch | the same, one transaction each | one burst |
| Write-back | `changes_requested` v1 per conversation, pushed in groups by the connector | the same | the same |
| Locks | 100 rows at a time | fine | long locks on hot rows |

**Chosen: a.** `conversations.apply_rule` (job, queue `default`, low
priority, payload: workspace, run id) takes one batch of 100 conversations
by keyset over the likeness index, in one transaction: re-checks that the
rule is active and its author may still change the inbox, skips suspicious,
member-placed and excepted conversations and those already so, records each
application with what it changed from (`before`), applies the change through
the same domain functions the operations use, publishes
`conversation_changed` and `changes_requested` per conversation and one
`rule_changed` with the run's progress, then re-enqueues itself with a
unique key `(run, cursor)`. Applications are unique per
`(rule, conversation)`, so a replayed batch changes nothing twice. The
total is counted when the run starts.

- **Write-back.** The connector already groups pending outbound changes per
  stream into `batchModify` calls (11 to 1,000 ids, 50 units) and paces them
  with the provider budget; a 1,000-message run becomes a handful of Gmail
  calls. Alternative considered: a v2 `changes_requested` carrying many
  conversations, to save one history stamp (1 unit) per conversation. Not
  worth a contract change now; revisit if the stamps show in the budget.
  Zoho mirrors archive and read only (labels and streams stay
  Campbooks-only, as today); Gmail's Receipts has no category.
- **Change feed without flooding.** Feed frames are ids, sequenced per
  workspace and collapsed by the sequencer; the client already batches
  invalidations every 50 ms. While a run is active for an inbox, the client
  refetches lists at most once per `rule_changed` progress frame.
- **Scale.** Batches of 100 keep each transaction short; 10,000 messages are
  100 jobs, fair with other workspaces through the queue's per-workspace
  claims.

## Decision 8: conflicts and exceptions

- **Two rules match one message**: per slot, the more specific one wins
  (Decision 3); different slots combine. `preview_rule` lists the active
  rules the new one would override on how many conversations, and the offer
  says so ("Replaces "Mail from shop.example → Receipts" for 40 of them").
  A rule with the same conditions and an action in the same slot replaces
  the old one (the old one is removed, without putting mail back).
- **A member moves one back** (moves it to another stream, unarchives,
  marks unread, removes the label, reopens a need the rule cleared):

  | | **a. Exception, then ask on the second** | b. Exception only | c. Ask every time |
  | --- | --- | --- | --- |
  | Friction | one question, only when it seems wrong | none | high |
  | Rule quality | improves: offers a narrower likeness that excludes both | drifts | improves |

  **Chosen: a.** The application is marked excepted and the rule never
  touches that conversation again; the operation's response carries
  `rule_exception { rule, recent_exceptions }` and the toast says "Back in
  Primary · Visor will leave this one alone" (Undo restores the rule's
  change). At the second exception within 30 days, an alert toast asks
  "You moved 2 back this month. Keep moving all mail from
  notifications@github.com to Updates?" with the narrower likeness that
  excludes both (1), **Remove the rule** (2) and **Keep it** (Esc). The move
  back still records the usual correction (overriding source `taught`).

## Decision 9: undo

- **Create** names its inverse: `create_rule` → `remove_rule` with
  `restore=true` (ADR 0013), which the client's Z and the toast call.
- **Remove with restore** marks the rule removed and starts a `revert` run:
  every application not reverted and not excepted, whose conversation still
  holds the value the rule set and was not changed by a member since, goes
  back to `before`. Same batching, progress and write-back as an apply run.
  An apply run still going is cancelled first.
- **Remove without restore** stops future applications only.
- Applications on arrival are reverted the same way.

## Decision 10: privacy and security

- Tables in schema `conversations` with forced RLS on `workspace_id`; every
  row carries `inbox_id`; inbox and workspace purges (`forget_connection`,
  `workspace_deleting`) delete rules, runs, applications and dismissals.
- Conditions hold personal data (addresses, list ids, subject words) and
  `origin_text` holds the member's words: never in events, job payloads,
  spans, logs or metric attributes. Events carry ids, enums and counts.
- Every operation checks the inbox role (and declares
  `x-campbooks.access = { resource: conversations.rule, action }` once ADR
  0017 lands); a rule the caller cannot see answers 404.
- A rule only does what its author may do: runs and arrival re-check that
  the author holds `sender` on the inbox; otherwise the rule pauses.
- Rules can't be used to hide phishing (Decision 3) and never delete,
  trash, send or forward.

## Data ownership

Schema `conversations` (all with `workspace_id`, forced RLS):

- `rules` (`id`, `inbox_id`, `teaching_id`, `conditions jsonb`,
  `actions jsonb`, `specificity int`, `origin`, `origin_text`, `state`,
  `taught_by`, `from_conversation_id`, `created_at`, `confirmed_at`,
  `removed_at`, `applied_count`, `last_applied_at`); index
  `(inbox_id) WHERE state = 'active'`.
- `rule_runs` (`id`, `rule_id`, `kind apply|revert`, `state
  running|done|cancelled|failed`, `cursor`, `total`, `done`,
  `skipped_suspicious`, `skipped_member`, `started_at`, `finished_at`).
- `rule_applications` (`rule_id`, `conversation_id`, `run_id` (null on
  arrival), `before jsonb`, `applied_at`, `reverted_at`, `excepted_at`);
  primary key `(rule_id, conversation_id)`.
- `rule_dismissals` (`inbox_id`, `user_id`, `likeness_key`, `action`,
  `count`, `dismissed_at`).
- `messages.subject_template text` (+ backfill job), indexes
  `(inbox_id, sender_address)`, `(inbox_id, (headers->>'list_id'))`,
  `(inbox_id, subject_template)`.
- `conversations.stream_source` and `decisions.source` accept `taught`;
  `needs.cleared_because` accepts `rule`. `attention.items.cleared_because`
  accepts `rule` (attention's own migration).

## Events

- `conversations.rule_changed` v1 (new, contract file): workspace, inbox,
  rule id, state, and the current run's kind, state, total and done. No
  conditions, no text. Declares an upsert of `conversations.rule` within the
  inbox, so only the inbox's members see it.
- `conversations.conversation_changed` and `changes_requested` (unchanged
  contracts) for every conversation a rule changes, origin `member`/`user`
  as for the operations (the rule acts on its author's behalf).
- `conversations.need_changed` v1: `cleared_because` gains `rule`
  (additive; attention is updated in the same change).

## Scopes and operations

| operationId | Method, path (under `/api/v1/w/{workspace_id}`) | Scope | Role | `x-campbooks` |
| --- | --- | --- | --- | --- |
| `conversations.suggest_rules` | GET `/conversations/{id}/rule-suggestions?action=…` | `conversations:read` | sender | read |
| `conversations.preview_rule` | POST `/inboxes/{inbox_id}/rules/preview` | `conversations:read` | sender | `mutates: false` |
| `conversations.create_rule` | POST `/inboxes/{inbox_id}/rules` | `conversations:write` | sender | undo `remove_rule`, automation, assistant |
| `conversations.confirm_rule` | POST `/rules/{rule_id}/confirm` | `conversations:write` | sender | undo `remove_rule` |
| `conversations.update_rule` | PATCH `/rules/{rule_id}` | `conversations:write` | author or manager | |
| `conversations.remove_rule` | DELETE `/rules/{rule_id}?restore=` | `conversations:write` | author or manager | `risk: low` (the client confirms in place) |
| `conversations.list_rules` | GET `/rules?inbox=&state=` | `conversations:read` | reader | |
| `conversations.get_rule` | GET `/rules/{rule_id}` | `conversations:read` | reader | |
| `conversations.dismiss_rule_suggestion` | POST `/conversations/{id}/rule-suggestions/dismiss` | `conversations:write` | sender | |

Errors (problem+json, catalogue codes in four locales):
`invalid_rule` (no conditions, a free-mail domain alone, two actions in one
slot, an unknown label or stream), `rule_not_active`, `inbox_read_only`, and
404 for a rule or conversation the caller cannot see.

## AI use

None of it needs a model. The engine is deterministic (`decide::taught`,
no role). It never calls `decide`, `embed` or `complete`; with no AI role
configured everything works. Rules are member-confirmed, so their answers
act; every change they make is undoable per conversation (the ordinary
operations, recorded as exceptions) and per rule (`remove_rule` with
restore). Later, #83 proposes rules from repeated confirmations: those are
`proposed` and act only after a member says yes.

## Does this copy the Rails app?

Partly, where it won. The Rails app's "Scout's memory" showed the inbox and
AI settings as sentences backed by real records, tagged taught, learned or
default, with confirm and remove for learned habits; this change keeps that
presentation because the domain map already defines Memory that way and it
is the plainest way to show a rule. Not copied: Rails' rules were made in
settings forms per behaviour (email rules, inbox group rules, tags), each its
own table and engine, applied to new mail only; here there is one rule model
for every producer, proposed from an action with a preview, run first in the
decide pass, applied to existing mail with progress and one undo.

## Slicing

One OpenSpec change. Built in this order, each step testable on its own:

1. `campbooks-ai`: subject template, `decide::taught` engine (conditions,
   slots, specificity, suspicious guard) with unit tests; eval harness unchanged.
2. `conversations` storage: migrations, `subject_template` at ingest +
   backfill, rule CRUD operations (`preview`, `create`, `confirm`, `update`,
   `remove`, `list`, `get`) and `rule_changed`; the API #121 compiles into.
3. The decide stage in `apply_changes` (taught answers, actions on arrival,
   applications), exceptions in the tidy and need operations.
4. Runs: `apply_rule` / revert, progress, write-back; `suggest_rules` and
   dismissals.
5. Client: the offer, progress and Undo, the row and reader lines, the
   exception question, Settings → Scout's memory.

If the PR grows too large, it splits into two stacked changes with their own
spec slices: (1)–(4) `taught-rules` (backend, terminal recording) and (5)
`teach-once-ui` (375px and desktop recording), agreed with the coordinator
first.

## Risks

- **A broad likeness hides something that matters.** Mitigations: hiding
  actions are never offered for correspondents; suspicious mail is never
  hidden; every handled row says so; Undo all; the exception question.
- **Write-back volume.** 10,000 changes at once on one Gmail account: the
  connector paces by budget, so the provider catches up over minutes while
  Campbooks already shows the result. The progress card says "Gmail catches
  up in the background".
- **Shared inbox surprise.** "For everyone" in the offer and the memory
  page, and the row line names the rule.
- **Template quality** across four languages: the normaliser is tested on
  real-shaped demo subjects in en, pt-PT, es-ES and fr; a template with fewer
  than two literal words is never offered.

## Open questions for the owner

Each has a recommended default; the implementation follows the default
unless the owner says otherwise.

1. **When to offer.** Right after the first action (recommended: the owner's
   "at least after the first"), or only once the member has done the same
   thing to two lookalikes?
2. **How broad by default.** For machine senders, the whole sender address
   (recommended: the owner's 1,000 notifications), or the narrowest likeness
   (the list or the subject template) with the wider one as a choice?
3. **Shared inboxes.** A rule taught by any sender acts for everyone in the
   inbox and only its author or a manager can change it (recommended), or
   only managers may teach?
4. **The name in copy.** The shipped UI says "Campbooks"; the prototype says
   "Visor" for what the assistant did and "Scout's memory" for the page.
   Which name does the assistant go by in these strings?
