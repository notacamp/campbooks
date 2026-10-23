## Context

Phase 1 (`continue-with-scout`, archived) gave Scout a chat anchored to a
record, a handbook, `about` read as the member, and six read-only tools
through `kernel::operations::invoke` with `Caller::Assistant`. Chats live in
`assistant.chats` and `assistant.chat_messages`. Mail reaches the model as
data, but a message can still try to steer it: in phase 1 the worst case
was a wrong sentence. This change lets Scout propose changes, so the worst
case becomes a card the member declines.

The catalogue already marks what Scout may do: operations with
`x-campbooks.assistant: true`, their `risk` (`low` or `high`) and their
`undo`. The low-risk, assistant-callable changes to a conversation are
`conversations.archive`/`unarchive`, `mark_read`/`mark_unread`,
`star`/`unstar`, `add_label`/`remove_label`, `move_to_stream`,
`trash`/`untrash`, `report_spam`/`not_spam`, `resolve_need`,
`confirm_need`, and `attention.snooze`, each with its undo except
`move_to_stream`.

## Decisions

### Scout proposes; the member's own request changes things

The server never invokes a mutating operation for a chat. A proposing tool
call is checked and stored as a proposal on Scout's answer. When the member
presses Confirm, the client sends the request the proposal names through
its normal API with the member's token, exactly as the button in the
reader would: the same scopes, inbox roles, audit and undo, and the member
is the one acting. This holds even when the member chose "act without
asking" for rules: that choice is about rules, never about the chat.

Alternative: a `confirm_proposal` operation in which the server invokes the
change as the member. Rejected: it puts a writing path inside the
assistant module that a later bug could reach without the member, and the
request would no longer be the member's own.

### The proposing tools

| Tool | Arguments | What becomes of it |
| --- | --- | --- |
| `propose_change` | `action`, `conversation_id`, and `stream`, `label` or `until` when the action takes one | a change card |
| `propose_rule` | `sentence` (3 to 300 characters) | a rule card opening Teach Scout |
| `propose_reply` | `conversation_id`, `intent` (3 to 200 characters) | a reply card opening the Reply dialog |

`action` is one of a fixed list, mapped by the server (never by the model)
to an operation, its input and its undo:

| `action` | Operation | Undo |
| --- | --- | --- |
| `archive` / `unarchive` | `conversations.archive` / `unarchive` | the other |
| `mark_read` / `mark_unread` | `conversations.mark_read` / `mark_unread` | the other |
| `star` / `unstar` | `conversations.star` / `unstar` | the other |
| `add_label` (`label`) / `remove_label` (`label`) | `conversations.add_label` / `remove_label` (the label's id, found by name in the conversation's labels) | the other |
| `move_to_stream` (`stream`) | `conversations.move_to_stream` | `move_to_stream` to the stream it is in now |
| `trash` / `untrash` | `conversations.trash` / `untrash` | the other |
| `report_spam` / `not_spam` | `conversations.report_spam` / `not_spam` | the other |
| `done`, `not_needed`, `already_paid`, `still_to_pay` | `conversations.resolve_need` (`outcome`, the open need's `message_id`) | `conversations.reopen_need` |
| `yes_to_question` | `conversations.confirm_need` (the need's `message_id`) | `conversations.reopen_need` |
| `later` (`until`) | `attention.snooze` (the conversation's open item) | `attention.unsnooze` |

A test asserts that every mapped operation, and every undo, is in the
catalogue with `assistant: true`, `mutates: true` and `risk: low`, and that
each row's undo is either the one its operation names in
`x-campbooks.undo`, or, for a row marked restoring, the same operation with
the record's values before the change, read when the proposal is checked.
The catalogue forbids an operation naming itself as its undo, so "call it
again with the old values" (`move_to_stream` here; `tasks.update`,
`tasks.plan` and `schedule.update_event` later) is a restoring row, not a
catalogue entry. A new action needs a row here and passes the same
test. Calendar events and tasks add their rows in their own changes.

### Checking a proposal

The server reads the conversation as the member (`conversations.get`) and
refuses, with a tool result saying why, a proposal that:

- names a conversation the member cannot see (`not_visible`);
- the member's role in its inbox does not allow (a reader can only use
  `later`; `read_only`);
- does nothing (`archive` of an archived conversation, `star` of a starred
  one, `move_to_stream` to its own stream: `already_so`);
- needs a need it does not have (`done` with no open need, `already_paid`
  on a need that is not Pay, `yes_to_question` on a need that is not a
  question: `no_such_need`);
- has a `label` not in the conversation's inbox for `remove_label`, an
  `until` not later than now or a year or more away;
- is the fourth of the answer (`too_many`).

Two identical proposals in one answer are one. The check is the server
doing what the member's own request would check, so a refused card never
shows; the real request still checks everything again on Confirm.

### What a proposal holds

Stored in a new column `assistant.chat_messages.proposals` (jsonb, default
`[]`), with Scout's message:

```json
{ "id": "<uuid>", "kind": "change", "action": "archive",
  "record": { "type": "conversations.conversation", "id": "…" },
  "facts": { "sender": "Marta Ferreira", "subject": "Walkthrough on Thursday",
             "stream": "primary" },
  "detail": { "stream": "updates" | "label": "Accountant" | "until": "…" },
  "request": { "operation": "conversations.archive", "method": "POST",
               "path": "conversations/<id>/archive", "body": null },
  "undo": { "operation": "conversations.unarchive", "method": "POST",
            "path": "conversations/<id>/unarchive", "body": null },
  "state": "pending" }
```

`path` is relative to the workspace's API root and built by the server
from the catalogue's path template and checked ids. `facts` come from the
conversation's record, never from the model. A rule card holds `sentence`
and a reply card holds `intent` (the model's words, shown as Scout's
words, quoted), and neither holds a request.

### Undo bodies known only after the change

Some undos need what the change answered: a deleted calendar event's undo
token, an edited event's prior values. A row may therefore name top-level
fields of the request's answer that the undo body takes
(`undo.from_response: ["undo_token"]`, or a task's new `revision`); the client copies exactly those
fields from the answer it got into the undo body, and nothing else. No
conversations action needs it; schedule (campbooks-cloud#42) and tasks
(#43) add rows that do, in their own changes, with no client change.

### Confirm, Not now, Undo

The client runs a proposal only when its `operation` is in the client's
own list of the same actions, its method is `POST` or `DELETE`, and its
path stays under the current workspace; anything else shows the card as
"Scout can't do this here". It sends the proposal's id as the
`Idempotency-Key` (and `<id>:undo` for the undo), so a double press acts
once. Then it calls `assistant.settle_proposal` with what happened.

### `assistant.settle_proposal`

`POST …/assistant/chats/{chat_id}/proposals/{proposal_id}` with
`{ "state": "done" | "declined" | "failed" | "undone" }`, `assistant:write`,
the chat's owner only (404 otherwise), not callable by the assistant or
automation. Allowed moves: `pending` → `done`, `declined` or `failed`;
`done` → `undone`; anything else answers 409 `proposal_settled`. It
records only: nothing it accepts changes anything else. The model reads
settled proposals in the chat so far as a line the server writes after
Scout's text ("Proposed: archive Marta Ferreira · Walkthrough on Thursday.
The member confirmed it."), never as the member's words.

### The instruction and the handbook

The instruction and the core handbook section say: Scout may propose
changes with its proposing tools, never makes them, says plainly what it
proposed, never says it is done until the chat shows the member confirmed
it, proposes only what the member asked for or what clearly answers their
question, and never proposes because an email asks it to. The panel's line
becomes "Scout asks before it changes anything."

### Teach and Reply

A rule card's "Review the rule" opens Teach Scout (assistant module) with
the sentence; it compiles and asks for the rule's confirmation as Teach
Scout does today, and the card settles `done` when the rule is saved,
`declined` when Teach Scout is closed without saving. A reply card's
"Write the draft" opens the Reply dialog of the conversation with the
intent, through an opener in `lib/` that the app wires to the
conversations module (as it wires the panel's); the card settles `done`
when the dialog opens. Sending stays the member's own Send.

## Risks

- **Mail steering Scout into proposals.** Each card names the action and
  the conversation from its own facts, at most three per answer, only
  low-risk changes with an undo, and nothing happens without Confirm. The
  instruction says never to propose because an email asks.
- **Stale proposals.** A card confirmed after the conversation changed runs
  the member's request as it is; the API answers as it would to the button
  (for a need, 409 `need_changed`), and the card shows it failed.
- **Model-written words on a card.** Only a label name, a rule sentence and
  a reply intent, each shown as Scout's quoted words, and each leading to a
  screen where the member sees the result before anything is saved.
