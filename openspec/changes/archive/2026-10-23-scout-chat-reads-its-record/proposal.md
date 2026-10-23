# Proposal

## Why

campbooks-cloud#345: in prod, the owner opened Scout on a conversation and
asked "what should I do?". Scout called `what_needs_me` and answered with
everything that needs them across the workspace, not the next step on that
thread. The conversation was in the context, but the instruction only said
`about` is "what the chat is about". It never said the member's questions
are about it, and the `what_needs_me` description ("what needs the member
now") matched the question.

## What Changes

- The chat instruction says that in a chat about a record, "this", "it" and
  questions with no object ("what should I do?", "what's next?") mean that
  record. Scout answers from `about` first (for a conversation: its need,
  its own reading of the newest message, the newest messages). It uses tools
  only for what `about` does not hold, or when the member asks beyond it, and
  never answers with a list of everything else.
- The core handbook section's "How to answer" says the same in one line (a
  new handbook version).
- The `what_needs_me` tool description says it covers all the member's mail,
  not what the chat's own record asks.
- The "chat question" log line carries `about`: the record type the chat is
  about, or `none`. Today the logs cannot tell a chat about a thread from a
  chat about nothing.

Modules touched: `assistant` (chat prompt, tools, handbook, log line). No API,
client or migration change.

## Impact

- Specs: `assistant-chat` (What Scout knows when it answers; Nothing personal
  in telemetry or logs).
