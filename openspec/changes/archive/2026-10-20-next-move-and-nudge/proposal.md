# Who has the next move: waiting on others, and Nudge

Modules: `campbooks-ai` (`decide`: two triage questions, the facts' veto on
who may be awaited), `conversations` (the `waiting` and `nudge` needs, hearing
back, the nudge draft and its text), `attention` (two reasons, the waiting
list), and the `attention` client module (Home). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/293, part of #164. No new
module and no cross-module dependency. Builds on the need clocks and Scout's
dates of `asks-lapse-or-go-stale` (campbooks-cloud#275, public PR #607),
which must be merged first.

## Why

The owner was copied on an email in which the sender asked a third person to
write to the owner. That person never wrote. The owner wanted to nudge them;
Scout offered to reply to the sender instead.

> Scout should be smarter in understanding what is the right course of action
> for an email, not just deciding simple actions like this.

Scout cannot do better today, whatever the model reads:

- **A need is only what the member owes** (`reply`, `do`, `pay`, `review`).
  "Someone owes the member" has no place to live, so the best answer for that
  email was no need at all, and the conversation went quiet.
- **Triage asks about the sender.** `needs_reply` asks whether the sender
  expects the recipient to write back. Nothing asks whose turn it is.
- **No Nudge.** The action catalogue (campbooks-cloud#166) holds Reply,
  Remind, Open attachment, Forward, Report, It's safe, Already paid and Still
  to pay. "Waiting on others → Nudge" exists only as a design-system story.

`copied-is-fyi` (#604) made copied mail read as FYI, which takes away the
wrong Reply. It does not give the right move.

## What changes

- **Two triage questions.** `next_move` (choice): who is expected to act
  next: the recipient, the sender, someone else on the email, or nobody.
  `awaited` (choice, asked only when the email has someone to pick): which
  of the other people on the email, as options built from its `To` and `Cc`,
  so the answer is always an address from the headers and never free text.
  The facts take people out of the options (the inbox itself, the sender,
  list, no-reply and service addresses): they only say no (ADR 0018).
- **Waiting on someone.** When the newest inbound message raises no other
  need and Scout reads that the sender or someone else on the email has the
  next move, the conversation gets a `waiting` need on that person. It shows
  nowhere in the queue. It carries a nudge time: the working day after the
  date the email gives for them (Scout's read of the email, as `due_on` is
  read today), else 3 working days after the email
  (`CAMPBOOKS_CONVERSATIONS__WAIT__WORKING_DAYS`).
- **Hearing back.** Mail from that person reaching the same inbox, in this
  conversation or a new one, clears the wait (`heard_back`). The member
  writing to them re-arms it from the send. Archiving does not end
  a wait.
- **Nudge.** At the nudge time the clock turns `waiting` into `nudge`, and
  Home lists it in a new last group, Nudge: "Ana Costa hasn't written yet",
  with João's line quoted. Its first action, Nudge (1), opens a dialog with
  a short follow-up Scout wrote (`conversations.write_nudge`, role
  `complete`) addressed to Ana in the same thread, João copied. The member
  edits and sends it with the usual undo, or writes their own. Heard from
  them (E), Later (S) and Not waiting (X) do the rest.
- **Waiting on others.** Home shows the conversations Scout is watching,
  with when it will check, so the member can see the work Scout is doing
  (`attention.list` `state=waiting`).
- **Reply only on the member's move.** Once `next_move` is measured and the
  policy accepts it, an accepted answer that is not the recipient keeps a
  model's `reply` or `do` from being raised for that message.
- **Mail already here.** On upgrade, inbound mail within the needs window
  that holds no open need is asked the two questions once, within the
  workspace's daily AI limit.

## Non-goals

- **Mail the member sent** ("I asked Ana for the plan on Monday"). Waiting on
  the people the member wrote to is the next slice; this one reads inbound
  mail only.
- Sending a nudge on its own: nothing is ever sent without the member's Send.
- Waiting on more than one person per conversation.
- A per-conversation wait chosen up front. Later (S) moves a nudge, and the
  default is a setting.
- Chips for waiting conversations in the inbox and the reader.
- Any rule that reads what an email says (ADR 0018).

## Impact

- `campbooks-ai`: `triage::NEXT_MOVE`, `triage::AWAITED`, the facts'
  `awaitable` parties, the questions' wording in the labelling guide.
- `conversations`: migration (`needs.awaited_name`, `needs.awaited_address`,
  `needs.nudge_at`, `needs.nudges`; the reasons `waiting` and `nudge`; the
  cause `heard_back`; an index for hearing back), the
  `nudge` draft kind, `conversations.write_nudge`, `ConversationSummary.need`
  gains `waiting_on`, the upgrade backfill. `conversations.need_changed` v1
  gains two reason values (additive; its only consumer is `attention`).
- `attention`: the reasons `waiting` and `nudge`, `state=waiting`.
- Client: the Nudge group, the nudge dialog, Waiting on others, in en, pt-PT,
  es-ES and fr.
- Evals: `next_move` and `awaited` labels on the mail-triage set, labelled by
  the owner.
