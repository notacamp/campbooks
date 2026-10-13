# Design

## Owner decisions (2026-10-13)

The owner approved this design and the prototype, answering the three open
questions:

1. **Archiving keeps Do and Pay.** Archiving a bill or an ask keeps it on Home
   until the member chooses Done or Not needed.
2. **Phishing warnings show** in Review even while `suspicious` is below its
   98 % precision target: a warning moves and hides nothing.
3. **Done in a shared inbox clears the need for everyone**: it is a fact about
   the conversation.

From the review: the selected item has no coloured stripe on its left edge;
selection is the selected background with a full focus ring.

## Context

- **The decide pass** (`conversations`, `campbooks_ai::decide`). Every new
  inbound message gets the rules' answers in the transaction that stores it
  (source `rules`), and the model's answers in `conversations.decide_message`
  when the `decide` role is configured (source `model`). Six questions:
  `stream`, `needs_reply`, `asks_action`, `has_deadline` (the evidence is the
  date's words: "Thursday", "30/10", "end of the month"), `urgency` (none,
  within days, today) and `suspicious`. Each answer is stored in
  `conversations.decisions` with its confidence, its reasons
  (`{rule, evidence}`) and whether the policy let it act.
- **The shipped policy** (`Policy::baseline`, from the mail-triage eval):
  rules answers act for `needs_reply` (from 0.88), `asks_action` (0.75),
  `has_deadline` (0.75) and `urgency` (0.75). `stream` and `suspicious`
  (97 % precision on `test` against a 98 % target) only record. Model answers
  never act (ADR 0009 note: Laya's zero-shot answers can be confidently
  wrong). "Acting" for the four questions above has meant nothing so far: no
  code reads them.
- **Production has no AI role configured.** Everything here must work on the
  rules alone.
- **Events carry no personal data** (ids, enums, dates, blob references), and
  deliveries of one event type are not ordered (ADR 0012).
- **The change feed** (#453) sends ids only, filtered by the read scopes and,
  inside a container such as an inbox, by the kernel's `Visibility` port.
- **The client** already has the approved Home pattern from the design system
  (#23): `HomeLayout`, `PageHero` with the Visor, `QueueGroup` per signal,
  `Item` with `ItemDetail` (a quote, context and actions), `Pill`, `Note`,
  the undo stack (Z), and signal colours for review, reply, decide, do and
  pay. The inbox (#459) shows stream suggestions with "Why?".
- **Copy**: the assistant is "Campbooks" in the product's copy; "Scout" is an
  internal word. Sentence case, plain words, four languages.

## Decision 1: what an attention item is

### Reasons

| Reason | Means | From the decide pass (this change) |
| --- | --- | --- |
| `review` | Look at this before you act on it | `suspicious`; and, with a model on, an answer the policy does not accept (a question to you) |
| `reply` | Someone is waiting for your answer | `needs_reply` accepted |
| `decide` | A choice is needed | nothing yet: schedule (invitations) and tasks will raise it |
| `do` | Someone asked you to do something | `asks_action` accepted, not a payment |
| `pay` | Money is owed | `asks_action` accepted with the `payment_due` reason |

**Deadline is an attribute, not a reason.** Options were (a) a "Deadline"
group of its own, or (b) a due date on any item. A deadline always belongs to
something to do (the rules give no deadline without an ask), so a separate
group would show the same message twice or hide what has to be done. Chosen:
(b). The due date orders each group and shows as a chip ("Due Thu",
"Overdue · was due Mon").

**One item per conversation.** A message can trip several questions ("Could
you confirm, and pay the deposit by Friday?"). Options: one item per reason
(the same message twice on Home) or one item with the strongest reason.
Chosen: one item, by precedence `review` > `pay` > `reply` > `do`, carrying
the due date and urgency of the whole message. Suspicious mail raises only
Review: a phishing "verify your mailbox" never becomes a Do.

### Who declares it

| | A. **The owning module declares a need; `attention` projects it** | B. `attention` projects from the raw decisions | C. A synchronous kernel port (`Attention::raise`) | D. No `attention` module: Home reads each module's needs |
| --- | --- | --- | --- | --- |
| Knows mail semantics | `conversations` only | `attention` (questions, rule ids, archive and reply rules) | `conversations` | `conversations` |
| Clearing (replied, archived, trashed) | where the facts are | `attention` re-learns conversation state from events | where the facts are | where the facts are |
| Rebuild by replay | yes | yes | no (writes are not events) | n/a |
| Atomic with the change | yes (outbox) | yes | yes | yes |
| Second producer (tasks, documents, money) | another event type to consume, same shape | another module's vocabulary inside `attention` | another port caller | Home merges N lists, N orderings, N "Later"s |
| New machinery | one module, one event per producer | the same, plus duplicated domain logic | a port and a module, no replay | a "needs" endpoint per module, merging in the client |

**Chosen: A.** It matches the domain map ("a reason declared by the module
that raised it") and keeps each domain's rules where its facts are. B makes
`attention` a second, drifting copy of every module's logic. C gives
atomicity we already have from the outbox and loses replay. D is genuinely
the cheapest for one producer, and is what Home would be if `conversations`
were the only source forever; it is rejected because tasks (M2), document
review and money (M3) all raise needs, Home must rank them together, "Later"
belongs to the member's list rather than to any one record, and notifications
need one place that decides a member should hear about something.

### What the item holds: references only

The question was where Home gets "Marta Ferreira", the subject and the quote.

| | α. **Items hold references; the producing module renders them** | β. The producer writes a note blob; `attention` copies display fields |
| --- | --- | --- |
| Personal data in `attention` | none | names, subjects, quotes (to purge and keep fresh) |
| Replay | from the event log alone | needs every note blob to still exist |
| Home requests | `attention.list` + one batch read per producing module | one |
| Freshness (a corrected name, a renamed party) | always current | stale until re-raised |
| Notifications later | the notice needs its text from somewhere (a note then) | ready |

**Chosen: α.** `attention` stays a small, generic projection with no personal
data that the retained event log can rebuild (and that the privacy work of
#65 never has to touch). Home makes two requests, not N: the client groups item
references by record type and asks each module's client for its rows in one
batch (`conversations.list` with `ids`). The cost is real but deferred:
when notifications arrive, the producer will attach a notice text blob to the
notice, not to every item.

### When it clears

`conversations` re-derives a conversation's need whenever its decisions or its
state change, and publishes the result. The table:

| Fact | Reply | Do | Pay | Review |
| --- | --- | --- | --- | --- |
| A message is sent from the inbox in the conversation after the one that raised it (from Campbooks or any other client; it reaches us as a `message_sent`) | cleared | cleared | stays | stays |
| The conversation is archived (here or at the provider) | cleared | stays | stays | cleared |
| Every message is in the trash or spam, or removed | cleared | cleared | cleared | cleared |
| A member chooses Done (`resolve_need`, `done`) | cleared | cleared | cleared | cleared |
| A member chooses Not needed (`resolve_need`, `not_needed`) | cleared, correction stored | same | same | same ("It's safe") |
| A new inbound message arrives | re-derived from the new message |
| The due date passed | stays, shown as overdue; leaves Home 7 days after the due date (still visible in the inbox chip) |

Two rows are judgement calls. **Archiving keeps Do and Pay**: archiving says
you have read it, not that you signed or paid; Home remembering is the point.
**Replying keeps Pay**: bills are not paid by email. The owner confirmed the
first (Owner decisions, 1).

Only messages sent in the last `CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS`
(default 14) raise a need, so a first sync of years of mail does not fill
Home with old questions.

### Undo

Every member action is an operation with its inverse (ADR 0013), pushed on
the client's undo stack (Z, and the toast):

| Action | Operation | Undo |
| --- | --- | --- |
| Done (E) | `conversations.resolve_need` `{ outcome: done }` | `conversations.reopen_need` |
| Not needed (X), "It's safe" | `conversations.resolve_need` `{ outcome: not_needed }` | `conversations.reopen_need` (the correction is kept, marked withdrawn) |
| Yes to a Review question | `conversations.confirm_need` | `conversations.reopen_need` |
| Later (L) | `attention.snooze` `{ until }` | `attention.unsnooze` |

### Who it is for

| | a. **Everyone who can see the inbox** | b. The addressee only |
| --- | --- | --- |
| Personal inbox (one member, the common case) | the member | the member |
| Shared inbox (`info@`) | every member; whoever replies or chooses Done clears it for all | nobody is the addressee: the inbox address is |
| Needs | the kernel's `Visibility` port, as the change feed does | a member ↔ address mapping we do not have |

**Chosen: a.** In Campbooks an inbox is one address shared by its members, so
the addressee *is* the inbox. Done and Not needed are facts about the
conversation and clear for everyone; Later is the member's own. A reader of a
shared inbox sees its items but gets no Done or Reply (the operations answer
403 `inbox_read_only`, and the client hides them). Picking one member for a
need is hand-off, which belongs to `tasks` (M2).

## Decision 2: trust and thresholds

**Display follows the policy.** Options:

| | i. Show only what the policy accepts | ii. **Accepted answers as items; everything else as Review, worded as a question or a warning** | iii. Show every answer, greyed when unsure |
| --- | --- | --- | --- |
| Rules only (production) | Reply/Do/Pay; no phishing warning (suspicious only records) | Reply/Do/Pay, and phishing warnings | noisy |
| Model on | the model changes nothing until its eval holds | the model's disagreements become questions, and the answers become labelled examples | the model's mistakes land on Home |

**Chosen: ii.** Reply, Do and Pay are something Campbooks asserts, so they
need an answer the shipped policy lets act. Review is, by definition, the
lane where Campbooks is not asserting: a warning ("This may be phishing")
or a question ("Does Jonas need a reply?") with a Yes/No that records a
correction either way. A phishing warning is shown even though `suspicious`
only records: a warning moves and hides nothing, and missing one costs more
than a false alarm. Review questions from the model need a confidence of at
least 0.8 to appear (a `Policy` entry, a reviewed code change like the rest
of the table); in production with no model there are none.

**Plain words, numbers folded.** A need's line is a sentence template in the
member's language, chosen by reason and by the rule that decided it, with the
sender's name:

- reply, `reply_asked`: "Marta Ferreira is waiting for your reply"
- reply, `question`: "Jonas Weber asked you a question"
- do, `request`: "Pedro Alves asked you to do something"
- pay, `payment_due`: "Acme Hosting sent a bill to pay"
- review, suspicious: "This may be phishing"
- review, a model question: "Does Jonas Weber need a reply?"

Under it, the sender's own sentence, quoted: "Could you confirm Thursday's
walkthrough? The builders need to know by Wednesday." and the chips (due,
waiting since). "How Campbooks decided" (closed by default, on Home and in the
reader) lists each answer in words, its source (rules or model), its
confidence and the threshold it was held to, and whether it acted. Nowhere
else is a number shown, so the reader's "Suggests Newsletters, 87 % sure"
loses its percentage too.

**The sender's sentence.** The rules match phrases ("could you confirm"), which
make poor quotes. `Reason` gains an optional `sentence`: the sentence of the
new part of the message (quoted history already excluded) that holds the
evidence, whitespace collapsed, at most 240 characters cut at a word. Stored
with the decision's reasons; no change to what the rules decide.

**Mail is data, never instructions.** The quote and the sender's name are
untrusted:

- shown as plain text in quotation marks, attributed ("Marta Ferreira,
  Mon 12 Oct"),
  never as HTML, never linkified;
- never an action: a quote that says "mark this done" or "ignore previous
  instructions" changes nothing, because no text from mail is read by anything
  that acts (templates and actions are fixed; the decide pass reads mail as
  its state, which is what it is for);
- on Review warnings, the sender is shown by address, since a display name is
  free to fake ("IT Support" from `it-support@at3lier-sol.example`), and the
  reasons come before the quote;
- when summaries arrive (`complete`), mail goes in as quoted data, the output
  is a sentence displayed and nothing else (no tool calls), and it is marked
  as written by Campbooks.

## Decision 3: Home "What needs you"

**Grouping.** Options: (a) groups by reason (the approved #23 pattern), (b) one
list by time ("Today", "This week", "Later") with reason pills, (c) a single
ranked list. Chosen: (a). Working one verb at a time batches well (answer the
three replies, then pay the two bills), it is the approved pattern, and time
is not lost: inside each group items run overdue first, then by due date,
then urgent ("today") before the rest, then the longest waiting first. The
groups are in a fixed order, Review first so a warning is never below the
fold, then Reply, Decide, Do, Pay; an empty group is not shown.

**The page.** The hero says how many things need you ("5 things need you"),
with the Visor `alert` when Review has something and `happy` when nothing is
left. Each item: who, the line, the quote (one line, the rest on Enter), the
chips. Enter opens the item in place with its actions and the "How Campbooks
decided" fold; the conversation opens with O or the subject link.

**Keyboard** (from the registry; existing ids where they exist): J/K move,
Enter opens, R reply (opens the conversation's composer), E done, L later
(Later today, Tomorrow, Monday, a date), X not needed, 1/2 for a Review
question's answers, Z undo. The brief suggested S for snooze; S is already
`conversations.star` in the item scope and L is the registry's "Later", so
Later is L.

**Done today.** A short list at the end of Home (the aside on desktop, a fold
on phones) of what left the list today, in sentences: "You replied to Marta
Ferreira", "You marked Pedro Alves's lease done · Undo", "Later: Acme
Hosting's bill, back Monday · Undo". Automatic clears (a reply, an archive)
have no Undo; member actions do.

**Empty state.** When the member can see an inbox and nothing needs them:
the Visor happy, "Nothing needs you", and a fact rather than a promise
("Campbooks read 12 new messages today. None asks anything of you."), with
Done today below. Before any mailbox, the first-run empty Home (#451) stays.

**Live.** `attention` publishes `attention.item_changed` declaring an upsert
of `attention.item` within the item's inbox, so the change feed's access
filter applies; the client invalidates `attention.list`. Need changes also
upsert the conversation, so inbox rows refresh their chips. A Later that
expires shows again on the client's next refetch (a timer to the earliest
`until`, and on focus).

**Inbox chips.** `ConversationSummary.need` (reason, trust, due date, overdue,
urgent, the quote) comes straight from `conversations`, so the inbox needs no
call to `attention`: "Needs a reply", "To do", "Pay", "Looks suspicious",
plus "Due Thu" or "Overdue". The reader's note leads with the line and the
quote, the chips, and Done / Not needed; "Why?" becomes "How Campbooks
decided".

## Decision 4: the log of what Campbooks did

Options:

| | **a. Defer the log to #89; Home's "Done today" is the only history** | b. A "What Campbooks did" list on Home from attention's own history | c. Resume #89 now on top of this |
| --- | --- | --- | --- |
| What it can say today | what left your list and why | "added 5 things to Home", "noticed a deadline" | the same, with pipeline detail |
| Undo | member actions (Done, Not needed, Later) | the same, plus "take this off Home" which is Not needed | the same |
| Cost | none extra | a second list that repeats the first | a module, a port, a page |

**Chosen: a.** In this change the only thing Campbooks does on its own is put
an item on Home, and every item already says why and carries its own undo
of Campbooks' judgement (Not needed, which also teaches). A separate log of
"added this to Home" would repeat the list next to it, the owner's own
complaint about #89. The log becomes worth reading once Campbooks acts on
its own (sorts under a policy that acts, drafts a reply, summarises a
thread); #89 is redesigned then, from those jobs, reading `attention`'s
cleared items as one of its sources.

## Decision 5: where drafts and summaries plug in later

A seam only:

- **The line is one slot.** The client composes a need's line from a template;
  `ConversationSummary.need` will gain an optional `summary` (text, marked as
  written by Campbooks, with the model) that replaces the template when
  present. The quote stays.
- **Reply opens a draft when there is one.** R opens the composer; when the
  need carries a draft id, it opens that draft instead. Drafts are already
  records of `conversations` (#101).
- **`decide` gates `complete`** (ADR 0009): only conversations with a Reply
  need are worth drafting, only long threads with a need are worth
  summarising. That gate is this change's need table, so the jobs that
  generate will read it and nothing else.

## Data ownership

### `conversations` (schema `conversations`)

```sql
CREATE TABLE conversations.needs (
    conversation_id uuid PRIMARY KEY REFERENCES conversations.conversations ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- The inbound message the need comes from.
    message_id uuid NOT NULL REFERENCES conversations.messages ON DELETE CASCADE,
    reason text NOT NULL CHECK (reason IN ('reply', 'do', 'pay', 'review')),
    trust text NOT NULL CHECK (trust IN ('sure', 'check')),
    basis text NOT NULL CHECK (basis IN ('rules', 'model', 'member')),
    -- The decision whose reasons explain it (its sentence is the quote).
    decision_id uuid REFERENCES conversations.decisions ON DELETE SET NULL,
    due_on date,
    urgent boolean NOT NULL DEFAULT false,
    state text NOT NULL CHECK (state IN ('open', 'cleared')),
    cleared_because text CHECK (cleared_because IN
        ('replied', 'archived', 'trashed', 'done', 'not_needed', 'superseded')),
    cleared_by uuid,            -- a user id when a member cleared it
    cleared_at timestamptz,
    -- Bumped on every change; consumers apply only newer revisions.
    revision bigint NOT NULL DEFAULT 1,
    updated_at timestamptz NOT NULL DEFAULT now()
);
```

Row-level security forced, like every table of the schema. The need is
derived by a pure function (`domain::needs::derive`) from the newest inbound
message's decisions (rules and model), the policy, the conversation's state
and its outbound messages, so it is unit-tested without a database. It runs
where those inputs change: `apply_changes` (new messages, archive and trash
from the provider), `decide_message` (model answers), the tidying operations
(archive) and `resolve_need` / `reopen_need` / `confirm_need`.

Corrections reuse `conversations.corrections`: `question` is `needs_reply`,
`asks_action` or `suspicious`, `from_value`/`to_value` are `true`/`false`, and
a confirmation stores the same value on both sides (a positive example).

### `attention` (schema `attention`, new)

```sql
CREATE TABLE attention.items (
    id uuid PRIMARY KEY,                       -- UUIDv7
    workspace_id uuid NOT NULL,
    record_type text NOT NULL, record_id text NOT NULL,       -- conversations.conversation
    container_type text NOT NULL, container_id text NOT NULL, -- conversations.inbox
    cause_type text NOT NULL, cause_id text NOT NULL,         -- conversations.message
    reason text NOT NULL CHECK (reason IN ('review', 'reply', 'decide', 'do', 'pay')),
    trust text NOT NULL CHECK (trust IN ('sure', 'check')),
    due_on date,
    urgent boolean NOT NULL DEFAULT false,
    since timestamptz NOT NULL,                -- when the cause arrived
    state text NOT NULL CHECK (state IN ('open', 'cleared')),
    cleared_because text,
    cleared_by uuid,
    cleared_at timestamptz,
    producer_revision bigint NOT NULL,
    UNIQUE (workspace_id, record_type, record_id)
);
CREATE INDEX items_open_idx ON attention.items (workspace_id, reason, due_on)
    WHERE state = 'open';

CREATE TABLE attention.snoozes (
    workspace_id uuid NOT NULL,
    item_id uuid NOT NULL REFERENCES attention.items ON DELETE CASCADE,
    user_id uuid NOT NULL,
    cause_id text NOT NULL,      -- a new message brings the item back
    until timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (item_id, user_id)
);
```

No names, addresses, subjects or quotes. `items` is a projection rebuilt by
replaying `attention.project_conversation_need`; `snoozes` is the member's own
write model and is kept across a rebuild. Row-level security forced on both.

## Events

**`conversations` publishes `conversations.need_changed` v1**
(`contracts/conversations.need_changed.v1.schema.json`), in the transaction
that changes the need:

```json
{ "workspace": "…", "inbox": "…", "conversation": "…", "message": "…",
  "revision": 4, "sent_at": "…",
  "need": { "reason": "reply", "trust": "sure", "due_on": "2026-10-15", "urgent": false },
  "cleared_because": null, "cleared_by": null }
```

`need` is null when cleared. Ids, enums and a date only. `changes()` upserts
the conversation within its inbox (inbox rows refresh their chips).

**`attention` consumes** `conversations.need_changed` (handler
`attention.project_conversation_need`: upsert by record, applied only when
`revision` is newer, so out-of-order and replayed deliveries are harmless),
`workspaces.workspace_deleting` (purge after the grace period) and
`workspaces.member_left` (drop that member's snoozes).

**`attention` publishes `attention.item_changed` v1** `{ workspace, item,
container }` when an item opens, changes or clears, and when a member snoozes
or unsnoozes it; `changes()` upserts `attention.item` within the container.

`conversations` consumes nothing new: Done, Not needed and the Review answers
are its own operations.

## Scopes and operations

| Operation | Route | Scope | `x-campbooks` |
| --- | --- | --- | --- |
| `attention.list` | `GET /api/v1/w/{workspace_id}/attention?state=open\|cleared` | `attention:read` | query; records `attention.item`; assistant |
| `attention.snooze` | `POST …/attention/{item_id}/snooze` `{ until }` | `attention:write` | mutates; undo `attention.unsnooze`; automation, assistant |
| `attention.unsnooze` | `POST …/attention/{item_id}/unsnooze` | `attention:write` | mutates; undo `attention.snooze` |
| `conversations.resolve_need` | `POST …/conversations/{conversation_id}/need/resolve` `{ outcome }` | `conversations:write` + `sender` | mutates; undo `conversations.reopen_need`; assistant |
| `conversations.reopen_need` | `POST …/conversations/{conversation_id}/need/reopen` | `conversations:write` + `sender` | mutates |
| `conversations.confirm_need` | `POST …/conversations/{conversation_id}/need/confirm` | `conversations:write` + `sender` | mutates; undo `conversations.reopen_need` |
| `conversations.list` (extended) | `GET …/conversations?ids=…` (1 to 100) | `conversations:read` | unchanged |

`attention.list` returns the caller's open items (or those cleared since the
start of today, for Done today), ordered as on Home, with counts per reason;
items in containers the caller cannot see are left out, and an item id the
caller cannot see answers 404 on the write operations. Items more than 7
days past their due date are left out at read time (no job). The web BFF
requests every catalogue scope, so the new scopes reach the web app at the
next sign-in; until then Home shows the empty state with a "sign in again"
note, as #89 planned for its own scope.

## AI use

- **Role**: `decide` only, and only through the answers the pass already
  stores. No new model call. `complete` is not used.
- **Without a configured role** (production today): the rules' answers drive
  Reply, Do, Pay and the phishing warning; there are no Review questions.
- **With a `decide` model**: its answers stay suggestions (the policy is
  unchanged); where they disagree with the rules, or the rules abstain, they
  become Review questions above the 0.8 floor.
- **Undo**: Not needed and Done are undoable, Later is undoable, and every
  Not needed is kept as a labelled example.

## Does this copy the Rails app?

Partly, and on purpose where it won. The Rails app had verb lanes (Reply,
Decide, Do, Pay, Chase, Nudge) and quoted the sender's own sentence on a
row; both are kept because the approved #23 prototypes use the verbs and
the quote is the most trustworthy explanation a rules-only system can give.
What is not copied: Rails refreshed a materialized standings table on a
10-minute timer and read asks out of every email with a generative model
call; here the need is projected from events with no timer, and found by the
decide pass's rules with no generation. Rails' lanes also lived on the
People page; here they live on Home, and the inbox is its streams.

## Slicing

One OpenSpec change, one PR (the owner's rule), built in this order so each
step is testable on its own: (1) the sentence and date resolution in the email
signals; (2) `conversations.needs`, `derive`, `need_changed`, the three need
operations and `ids` on `conversations.list`; (3) the `attention` module
(scaffolded with `cargo xtask new-module`), its projection, `list`, `snooze`,
`unsnooze` and the feed; (4) the inbox chips and the reader; (5) Home. Steps
(1)–(3) get a terminal recording and (4)–(5) a 375px and desktop recording,
both in the PR.

Next changes, each its own PR: `attention-notices` (`NoticeRaised` once the
notifications module exists), the drafts and summaries that fill the seam
(#20, #47), tasks' needs and hand-off (M2), and #89 redesigned on top.

## Risks

- **Rule recall.** The rules miss asks phrased unusually, and a missed reply is
  silent. Mitigation: the inbox stays the source of truth, and with a model on,
  disagreements surface as Review questions.
- **Shared inbox.** Done clears for everyone; in a shared inbox that may
  surprise a second member. Mitigation: Done today names who did it.
- **Dates.** "Thursday" resolves to the next Thursday after the message was
  sent, in UTC; an ambiguous "03/10" resolves day-first (the four locales'
  convention) and is shown with the words it came from.

## Open questions for the owner

None: answered on 2026-10-13 (see Owner decisions).
