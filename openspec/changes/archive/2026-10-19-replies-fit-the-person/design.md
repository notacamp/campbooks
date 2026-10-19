# Design

## 1. Alternatives

a) **A background style profile per correspondent**, summarised by the model
and stored. An extra model call per person, stale as soon as the member's
habits change, and it stores derived personal data about someone who never
agreed to be profiled. Rejected.

b) **A formality choice per reply** (a toggle). The owner asked that this not
be visible: the reply should simply fit. Rejected.

c) **Retrieve the member's own recent sent mail to that person at request
time and send it as style examples.** No extra model call, always current,
nothing new stored, bounded. Chosen.

## 2. What is read

Only for `write_reply`, in the transaction that already reads the thread,
from the conversation's own inbox:

- `conversation.total_messages`: the conversation's message count, capped at
  1000, so the model knows how deep it is beyond the eight shown.
- The person answered: the sender of an inbound message, or the first `to`
  recipient of one the member sent. None (an outbound message with no `to`):
  nothing person-specific is sent.
- `them`: shown name, whether the inbox wrote to them in its newest 500 sent
  messages (any conversation, this one included), and, for the last 365 days
  outside spam and trash, the count of their mail to the inbox and of the
  inbox's mail to them, each capped at 100.
- `my_recent_messages_to_them`: up to three of those 500 sent messages that
  went to or copied them, from other conversations, in the last 365 days,
  not in the trash. Each is its own words (the same reduction as thread
  messages), at most 1.5 KiB, 4 KiB in all; the newest are kept and sent
  oldest first. Only the time and the text: no subject, recipients or names.
  The key is left out when there are none.

The address is matched the way `rules::facts::corresponds` does, as a whole
JSON string in the lower-cased recipients, over the newest 500 outbound rows
served by `messages_outbound_idx`, so a mailbox with years of mail costs one
bounded index scan. Their inbound mail is served by
`messages_inbound_sender_idx`.

## 3. Privacy

- Same inbox only. It is the inbox the reply is sent from, which the caller
  was just authorized to read and send from. Another inbox of the workspace,
  even one the member can see, is never read for this.
- Past mail goes to the same `complete` role, under the same
  external-content check, as the thread already does; nothing new leaves.
- It travels as JSON data, never in the instruction, and the instruction
  says it is for style only: no dates, amounts, names or commitments from it.
- Nothing is stored: not the profile, not the counts, not the examples.
- Few messages, bounded in size and age, and none from the trash.

## 4. Cost

Up to 4 KiB of examples plus under 0.5 KiB of counts and name per written
reply (roughly 1,200 tokens at most). No extra call. Three small bounded
index queries in the transaction that already exists.

## 5. AI use

Role `complete`; when it is not configured or may not read mail, the reply is
unavailable (409 `reply_unavailable`) exactly as before and the member writes
it. The result is a draft the member reads, edits and sends; nothing is sent
for them.
