## Why

The decide pass already reads every inbound message and records whether it
needs a reply, asks for something, names a deadline, is urgent or looks
suspicious, with its reasons. Only the stream answer reaches the member, as a
suggestion in the inbox. The rest is stored and never used, so Home still says
"Nothing needs you yet" while the inbox holds questions, bills and phishing.

The owner looked at the assistant activity prototype (campbooks-cloud#89) and
paused it: a log of sorting alone is of little use, and a log of pipeline
stages is hard to read. The decision (2026-10-13) is to give Campbooks real
jobs first. The first job is Home showing what needs each member, with chips
in the inbox. Drafts and summaries come later, once the `complete` role is
configurable (#20, #47).

Production runs with no AI role configured, so this has to work on the
deterministic rules alone and get better when a `decide` model is on.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/39.

Modules touched:

- **`attention`**: a new module (crate, schema, client module), already listed
  in `docs/domain-map.md` for M1. It depends on the kernel only, so it needs
  no `architecture.toml` entry.
- **`conversations`**: works out the *need* of each conversation from its
  decisions and its state, publishes it, and owns resolving it (Done, Not
  needed) with undo.
- **`campbooks-ai`**: the email signals keep the sender's sentence that a
  reason matched, and resolve a deadline's words to a date.
- **Client**: Home, the inbox rows and the reader.

No cross-module dependency: `attention` consumes `conversations` events
(ADR 0012) and reads containers through the kernel's `Visibility` port.

## What Changes

- **A conversation's need.** `conversations` turns accepted decide answers
  into at most one need per conversation: Reply, Do, Pay or Review, with a due
  date when the deadline's words resolve to one, and whether it is urgent. It
  is cleared by facts it already knows (you replied from any client, you
  archived it, it went to the trash or spam) and by the member (Done, Not
  needed). Old mail never raises one: only messages from the last 14 days.
- **Trust, read plainly.** Reply, Do and Pay come only from answers the
  shipped policy lets act. Suspicious mail and, when a model is on, answers
  the policy does not accept go to Review, worded as a warning or a question.
  No percentage is shown up front; numbers sit behind "How Campbooks decided".
- **The sender's own sentence.** The email signals keep the sentence a reason
  matched, from the new part of the message only. Home and the reader quote it,
  as quoted text, never as an instruction.
- **The `attention` module.** A projection of every module's needs, with no
  personal data: what needs whom, why, and by when, as record references. It
  owns the member's own "Later" (snooze). Operations `attention.list`,
  `attention.snooze` and `attention.unsnooze`; scopes `attention:read` and
  `attention:write`; live through the change feed.
- **Resolving a need.** `conversations.resolve_need` (Done or Not needed) and
  its undo `conversations.reopen_need`, plus `conversations.confirm_need` for
  Review questions. Not needed and confirmations are stored as corrections,
  the labelled examples the decide evals need (campbooks-cloud#59).
- **Home: What needs you.** Groups in a fixed order (Review, Reply, Decide,
  Do, Pay), each sorted by due date first. J/K, Enter, R reply, E done,
  L later, X not needed, Z undo. A "Done today" list says in plain sentences
  what left the list and offers Undo. An empty state when nothing needs you.
- **Inbox chips.** Rows and the reader carry the need: "Needs a reply",
  "Due Thu", "Pay by 30 Oct", "Looks suspicious". The reader's note says why
  in a sentence and quotes the sender.
- **A seam for drafts and summaries.** The line that says what a need is about
  is one replaceable slot, and Reply opens a draft when one exists. Nothing
  generates text in this change.

## Non-goals

- The activity log of everything Campbooks did (#89 stays paused). Home's
  "Done today" covers what left the member's list; a fuller log waits until
  Campbooks takes actions of its own (sorting under a policy that acts,
  drafts, summaries).
- Drafts, summaries and any use of the `complete` role (#20, #47).
- The Decide reason from mail: nothing in the decide pass detects a choice
  yet. The group exists for the schedule and tasks modules (invitations,
  asks) and for a later decide question.
- Tasks, hand-off and assigning a need to one member of a shared inbox
  (`tasks`, M2). A need is for everyone who can see its inbox.
- Notifications, push and digests (`NoticeRaised`, the notifications
  module).
- Waiting on others ("you wrote last"), and nudges.
- Trash and delete from Home, and a "report phishing" action.
- A member time zone: dates resolve against the message's sent date in UTC
  until accounts carry a time zone.
- Changing the decide policy table or the rules' confidences.
