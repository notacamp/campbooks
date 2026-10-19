# Tasks

## 1. Reading

- [x] 1.1 The answered message's direction, sender and recipients, and the inbox id, carried out of the authorization transaction
- [x] 1.2 The conversation's length, the person's name and capped counts, and up to three earlier messages to them from the same inbox
- [x] 1.3 Non-macro queries bounded by `SENT_SCAN`, 365 days and the byte budgets

## 2. Prompt

- [x] 2.1 `conversation.total_messages`, `them` and `my_recent_messages_to_them` added to the written reply's input only
- [x] 2.2 The instruction matches the register and uses earlier mail only for style

## 3. Tests

- [x] 3.1 Prompt unit tests: new keys only in the user message, only for a written reply, instruction wording
- [x] 3.2 HTTP tests: newest three to the person, same inbox only, not this conversation, someone new, answering my own message, bounds, suggestions unchanged, long conversation

## 4. Close

- [x] 4.1 OpenAPI regenerated: no change
- [x] 4.2 Client types regenerated: no change
- [x] 4.3 CHANGELOG updated
- [x] 4.4 Gates green
- [x] 4.5 UI verified: no UI change
