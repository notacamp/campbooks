## Summary

Scout's written replies now fit the person: as formal or as casual as the member's past emails to them, and as the conversation has become.

**Before:** the model got the subject, the newest 8 messages, the member's name and language.
**After (written replies only):** also `conversation.total_messages`, `them` (shown name, whether the inbox wrote to them before, capped counts each way over the last year) and up to 3 of the same inbox's own sent messages to them from other conversations (`my_recent_messages_to_them`, own words, 1.5 KiB each, 4 KiB total, oldest first). The instruction says to match greeting, sign-off, form of address, length and warmth, to follow the other person's tone when there is no history, to drop formalities deep into an exchange, and to use earlier mail only for style, never as facts. Likely replies are unchanged.

## Privacy boundary
Same inbox only (the one the reply is sent from, already authorized for read and send). Last 365 days, newest 500 sent rows scanned, not trash, nothing stored, sent as JSON data under the existing external-content check. No extra model call, about +4.5 KiB per write.

## Tests
Prompt unit tests (keys only in the user message, hostile text stays data, instruction wording) and HTTP tests: reads how I wrote before, past mail stays in its inbox, this conversation not repeated, someone new, answering my own message, bounds, suggestions get nothing, long conversation count.

## OpenSpec
`replies-fit-the-person`, validated and archived in this PR; baseline `conversations` spec updated.

## Changelog
Entry file `changelog/unreleased/changed/271-replies-fit-the-person.md`.

## Recording
Backend-only: the new reply tests passing in CI (run on 6ccefd86), the shape of the new model input (fictional Marta), and the commit stat.

[![replies-fit-the-person — click for full video](https://files.not-a-camp.com/pr/campbooks/603/93d91e153aa5ba42825911660fd7a79e/rec.gif)](https://files.not-a-camp.com/pr/campbooks/603/93d91e153aa5ba42825911660fd7a79e/rec.mp4)

Closes notacamp/campbooks-cloud#271

🤖 Generated with [Claude Code](https://claude.com/claude-code)
