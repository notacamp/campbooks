## Why

campbooks-cloud#271. Scout's written replies read only the conversation, so
they greet and sign the same way for a new client and for a colleague the
member has written to a hundred times, and they stay as formal in the
twentieth message of an exchange as in the first. How a person writes
to someone depends on who it is and how the exchange has gone. The model
can see neither unless it is told.

## What changes

- A written reply (`conversations.write_reply`, drafting and rewriting)
  also receives, as data: the length of the conversation, the person
  answered (shown name, whether the inbox wrote to them before, capped
  counts of mail each way in the last year), and up to three of the same
  inbox's own sent messages to them from other conversations, for the
  register only.
- The instruction tells the model to match the greeting, sign-off, form of
  address, length and warmth to how the member wrote to that person, to
  follow the other person's tone when there is no earlier mail, to drop
  formalities deep into an exchange, and to use earlier messages only for
  how to write, never as facts.
- Likely replies (`conversations.suggest_replies`) are unchanged.

## Capabilities

### New Capabilities

### Modified Capabilities

- `conversations`: what a written reply is allowed to send to the drafting
  model.

## Impact

- Conversations: `features/reply` (`mod.rs`, `prompt.rs`, tests).
- No migration, no new endpoint, no OpenAPI change, no stored data.
- Up to about 4.5 KiB more input per written reply; no extra model call.

## Non-goals

- A formality toggle or any new control for the member.
- Storing a style profile, or any derived data about a correspondent.
- Reading another inbox, or mail older than a year.
- Changing likely replies.
