# Design

## Context

Compile (`assistant.compile_instruction`, `assistant:write`) is the only
caller of the `complete` role. It sends the member's sanitised sentence, their
inbox addresses, the stream keys and their label names, with a fixed
instruction and a strict JSON schema, and checks what comes back
(`validate.rs`). Until now, an answer that did not parse became the outcome
`unclear`, the same as a sentence the checks could not turn into a rule.

Data ownership is unchanged: compile stores nothing; rules are created in the
conversations module through `conversations.create_rule`, as before. No event
is published or consumed. No endpoint, scope or migration is added.

## Decisions

### Budget for a reasoning model

`max_tokens` goes from 800 to 4096. A reasoning model spends its thinking
from the same budget before it writes the answer; the answer itself is at
most three small rules. Temperature stays 0 and the schema stays strict, so
the answer stays small whatever the budget.

### One code fence, nothing more

`ChatResponse::json` trims the content and, when it starts with ` ``` ` and
ends with ` ``` ` and the opening line's info string is empty or `json`,
parses what is between. Prose around a fence, another info string, a fence on
one line, or JSON cut short are still errors. This is text hygiene on the
model's own answer, not a decision about mail (AGENTS.md, no content rules).
`decide` does not use this path (it has its own protocol), so only compile is
affected.

### Whose failure

An answer that does not parse is the model's failure: 503 `compile_failed`,
which the page already shows as "Scout couldn't read your sentence just now.
Try again in a moment, or build the rule yourself." `unclear` stays for a
well-formed answer whose rules the checks dropped, or that says so. The member
is never told to reword a sentence the checks never saw.

### What the log says

The `compile` info line adds `finish_reason` (one of `stop`, `length`,
`content_filter`, `tool_calls`, `other`, `none`, never the provider's raw
string), `completion_tokens` and `content_chars`. When the answer does not
parse, a warning gives `reason` (`empty`, `truncated` when the model stopped
for length, `not_json`) with the same sizes. Neither ever holds the sentence,
the answer or the parser's message, which can quote the answer.

### AI use

Role `complete`, as before. When it is not configured or may not receive
content, compile answers 409 `compile_unavailable` and the page shows the
builder, as before. Nothing is saved until the member presses Save, and every
saved rule has Undo.

### The builder

`TeachScout` holds why the builder is shown: `chosen` (the member's button,
no model call), `failed` (after an error) or no model (409). With a model, the
builder has "Tell Scout in a sentence instead". The builder's "It's from"
condition is `SenderFilter` (search by name over `conversations.list_senders`,
a typed address or domain as its own card, free mail domains refused). Each
ticked sender is read with `readSender` into `sender_address` or
`sender_domain`, one rule each: conditions are ANDed, so "any of these" is
several rules. Each rule gets its own `RuleSave` card (preview, inboxes, Save,
Undo); the first answers ⌘↵. Once every card is saved or cancelled, the form
is back, empty.

## Risks

- A larger budget costs more on a model that thinks long; the hourly limit
  (20 per member) and the workspace's daily AI limit still apply.
- A model that ignores the schema in some other way still fails; the log now
  says how, so the next fix is informed.
