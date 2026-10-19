# Design

## The frame: whose move is it?

Every need so far answers "what does the member owe?". The owner's email
asks a different question, "whose turn is it?", and the answer decides the
action: the member's turn gives Reply, Do or Pay; someone else's turn gives a
wait, and later a Nudge; nobody's turn gives nothing. Asking it once per
message, in the triage call that already runs, gives every later action the
same footing instead of adding one-off questions per action.

## Alternatives weighed

| Option | Verdict |
|---|---|
| A rule: copied + "please email" in the body → wait | Rejected. A content rule (ADR 0018); breaks on the next phrasing and the next language. |
| A `noul` per case (`awaits_third_party`, `sender_promised`) | Rejected. Two yes/no answers can both be yes; a choice keeps one move per message and calibrates as one question. |
| Ask the `complete` model (Scout's summary call) who is awaited | Rejected for the decision. `complete` is free text and uncalibrated; `decide` gives typed answers with probabilities, measured by `cargo xtask eval decide`. `complete` is used only for the date, as `due_on` already is (#607). |
| `next_move` (choice) + `awaited` (choice over the email's own people) in the triage call | **Chosen.** Typed, calibrated, measured, and the awaited person can only be someone the headers name. |
| Wait on outbound mail too | Next slice. Needs a `decide` call on sent mail (a new cost line and a new eval set). |

## Questions

`next_move`, a `choice`:

- `recipient`: the recipient is asked to reply, decide, do or pay something.
- `sender`: the sender said they will send or do something next, or is
  expected to come back to the recipient.
- `someone_else`: someone else on the email is asked to do something for the
  recipient or to get in touch with them.
- `nobody`: it informs, thanks or settles the matter.

`awaited`, a `choice`, asked only when at least one person is awaitable:
"If someone other than the sender and the recipient is expected to act next,
who?" Options `p1` … `p6`, each described as `Name <address> (To|Cc)` in the
envelope's order (To before Cc), plus `none`. The stored answer keeps its
options, so `p2` maps back to the address it named when it was asked.

**Awaitable** (facts, deterministic, only removing people): not one of the
inbox's addresses, not the sender, not a list, no-reply or service address
(`is_machine_sender`), deduplicated without regard to case, at most six. A
sender that is itself a no-reply or service address is never awaited either.

Adding questions leaves the stored answers valid; `CONTEXT_VERSION` is not
bumped (its rule). Both questions are catalogue-like for the call's strict
checks: an answer missing or outside its options records nothing and fails
nothing. Check the provider's limit on questions per call before adding
them (nine with `awaited`, plus the catalogue's).

## Policy and trust

`next_move` and `awaited` are not measured yet, so the shipped policy only
records them, as it did `money`. Until they are measured:

- a `waiting` need comes from a model `next_move` of `sender` or
  `someone_else` with a confidence of at least 0.8, and, for `someone_else`,
  a model `awaited` answer naming a person (not `none`);
- its trust is `check`: Not waiting (X) stores a correction, which is the
  labelled data the refit needs (campbooks-cloud#180);
- `next_move` never takes away a `reply` or `do` need.

Once the owner's labels are in the committed report, `Policy::baseline` gets
an operating point for `next_move`. From then on, an accepted `sender`,
`someone_else` or `nobody` keeps a **model's** `reply` or `do` answer from
raising a need for that message (the facts and taught rules still win), and
the `waiting` need is `sure`.

## Precedence

One need per conversation. From the newest inbound message, in order:
`review`, `pay`, `reply`/`do` (sure, then check), then `waiting`. A wait is
only raised when the message asks nothing else of the member, and only for
mail the facts do not mark as list, automatic or no-reply, whose accepted
stream is `primary` or which has no accepted stream answer.

## The wait's lifecycle

```
                     inbound msg: someone else has the move
                                   │
                                   ▼
        ┌────────────── waiting (not listed) ◄─────────────┐
        │                          │ nudge_at passes        │ member writes to them
        │                          ▼ (need clock)           │ (re-armed
        │                 nudge (listed: Nudge group) ──────┘  from the send)
        │                          │
        ▼                          ▼
  heard_back: mail from them   E Heard from them (resolved) · X Not waiting (correction)
  reaches the inbox            S Later (attention snooze, per member)
                               trashed / spam / a newer inbound message (superseded)
```

- **`nudge_at`**: when Scout's read of the email (the `complete` summary call
  of #607, which runs when a need opens) gives `due_on` for the awaited
  person, the working day after it; else the email's sent time plus
  `CAMPBOOKS_CONVERSATIONS__WAIT__WORKING_DAYS` (default 3, 1 to 15) working
  days. Working days are Monday to Friday by UTC date, at the time of day
  the email was sent. Re-armed from a send the same way, from the send's
  time.
- **The clock** is #607's `conversations.need_clocks`: a `waiting` need's
  clock row is its `nudge_at`. `conversations.wake_needs` turns it into
  `nudge` (a new revision, `need_changed` published), so Home hears it
  through the change feed like any other change.
- **Hearing back** runs where inbound mail is stored: an inbound message
  whose sender address equals an open `waiting`/`nudge` need's
  `awaited_address` in the same inbox (case-insensitive), sent after the
  need's message, clears it as `heard_back`. In the same conversation the
  new message is then read as usual. Index:
  `needs (inbox_id, lower(awaited_address)) WHERE open AND reason IN
  ('waiting','nudge')`.
- **Re-arming**: a message sent from the inbox with the awaited address in
  `To` or `Cc`, after the need's message, sets the need back to `waiting`
  with a new `nudge_at` and `nudges + 1`: a new revision with a need, not a clear. A Nudge sent from
  Home is such a message.
- **Not cleared by archiving**: archiving tidies the inbox; the member is
  still waiting.
- **Never dropped by age**: a `nudge` item stays listed until it is answered,
  like a bill. #607's "old asks become a question" applies to `do` and
  `reply` only.

## Data ownership and events

- `conversations` schema: `needs.awaited_name`, `needs.awaited_address`
  (the person, held with the conversation like its messages' addresses),
  `needs.nudge_at`, `needs.nudges`; the reason and cause checks widened (and
  validated in a second migration, as #607 does). Purged with the
  conversation and the workspace like every row of the schema.
- `conversations.need_changed` v1: the reason enum gains `waiting` and
  `nudge`, the cause enum `heard_back`; the due date is
  `nudge_at`'s date. No name or address travels on the event (AGENTS.md:
  personal data stays out of the log). Additive: old events replay; the only
  consumer, `attention.project_conversation_need`, changes in the same PR.
- `attention` schema: the reasons widen. `attention.list` leaves `waiting`
  items out of the queue; `state=waiting` returns them, soonest nudge first.
- `ConversationSummary.need.waiting_on`: `{ name, address }` of the awaited
  person, read only through `conversations.list` (Home's batch read), so
  Home can say "Ana Costa hasn't written yet".

## Endpoints and scopes

- `attention.list` `state=waiting`: `attention:read`. No new endpoint.
- Draft creation gains the kind `nudge` (existing operation,
  `conversations:send`): the reply-all of the need's message with the
  awaited person in `To` and everyone else of reply-all in `Cc`, the inbox's
  own address in neither; subject, `In-Reply-To`, `References` and thread
  key as a reply. Only for a conversation whose open need is `waiting` or
  `nudge`; else 409 `not_waiting`.
- `POST …/conversations/{conversation_id}/nudge/draft`
  (`conversations.write_nudge`, `conversations:send`): Scout's text for the
  nudge, or the member's draft rewritten shorter or as asked. Same rules as
  `conversations.write_reply`: `complete` role only where it may read mail
  (else 409 `reply_unavailable`), 404 to who may not read, 403
  `inbox_read_only`, the daily AI limit as interactive and the per-member
  hourly allowance, 503 `reply_failed`, changes nothing, mail as data in
  JSON strings. The instruction adds: it is a short, friendly follow-up to
  the awaited person, about what they were asked for, in the language of
  the thread, never pushy, no invented facts or dates.

## AI use

- Role `decide` for `next_move` and `awaited`, in the triage call that
  already runs (two more questions, no new call). Without a checked `decide`
  role nothing is asked and no wait exists.
- Role `complete` for the nudge date (the summary call that already runs
  when a need opens) and for the nudge's text. Without it: the date falls
  back to 3 working days, and the dialog offers "Write my own" (an empty
  nudge draft, addressed) with the line saying Scout can't write here.
- Undo: Not waiting (X) clears with Undo (Z); a sent nudge has the send's
  undo window; nothing is ever sent without Send.

## Cost

Two more choice questions in an existing call: a few hundred prompt tokens
per inbound message with people to pick, fewer without. The upgrade asks
them once for recent mail with no open need, inside the daily AI limit. To be
measured per 1,000 emails before the flag is switched on, as #166 was.

## Rollout

Behind the catalogue's switch (`CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED`): off, no
question is asked and no wait is raised. The owner switches it on in cloud
after reviewing the recording.
