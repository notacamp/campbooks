## Context

Phases 1 and 2 built the chat, its about registry (record type → read
operations invoked as the member, a projection, bounds), read-only tools,
proposals and the panel, opened through `lib/scout-chat` with a
conversation id. A chat has `about_type` and `about_id`, both null for a
chat about nothing; the unique key is on (workspace, member, type, id)
where the id is not null.

## Decisions

### More about types

| Type | Read as the member | Projection |
| --- | --- | --- |
| `conversations.rule` | `conversations.get_rule` | its sentence parts (conditions, actions), inbox, origin, state, created, how many conversations it acted on |
| `conversations.rule_notice` | `conversations.get_rule_notice` (new) | kind, sender (name, address), its rule, its counts, shared |

`conversations.get_rule_notice` is `GET …/rule-notices/{notice_id}`,
`conversations:read`, a query callable by the assistant, answering one
open suggestion as `list_rule_notices` lists it, for an inbox the caller
may write in, else 404. It does not start a notice pass.

A chat about nothing (`about` omitted) is always a new chat: there is
nothing to continue by key. Its context says so, and the tools are how
Scout finds things.

### The opener takes a record

`openScoutChat(about: RecordRef | null, options: { from?, ask? })`, so
any module opens a chat about any registered type, or about nothing with a
first question. Phase 1's `openScoutChat(conversationId, from)` callers
move to it.

### Entry points

| Where | About | Button |
| --- | --- | --- |
| Scout noticed (Home) | the notice | "Ask Scout" in its actions |
| Teach Scout, after Save rule | the rule | "Ask Scout about this rule" |
| Rules list (settings) | the rule | in the row's overflow |
| Reply dialog | its conversation | icon button with the visor, tooltip "Ask Scout" |
| Nudge dialog | its conversation | same |
| Waiting on others row | its conversation | in the row's actions |
| ⌘K | nothing | "Ask Scout" item; with typed text, "Ask Scout: <text>" |

Worth a look needs are Home needs and already have "Continue with Scout"
from phase 1.

Opening the panel from a dialog closes nothing: the panel opens beside it
from `lg` up and over it below (the dialog stays where it was when the
panel closes).

### Recent chats

`GET …/assistant/chats` (`assistant.list_chats`, `assistant:read`, the
caller's own chats only, newest message first, at most 30, a query not
callable by the assistant) answers for each: id, about (type and id or
null), the first question (the member's own words, at most 120
characters), questions asked, last message time. The panel's header gets
"Recent" (a menu); choosing one opens it. The list shows the first
question and the time, never a subject read again: a chat about a record
the member lost still lists, and opening it says Scout can no longer see
it, as phase 1 does.

### Starter questions per type

- Rule: "What does this rule do?", "What has it changed?"
- Rule notice: "Why do you suggest this?", "What would it change?"
- Nothing: "What needs me today?", "What am I waiting on?"
- Conversation: as phase 1.

### The panel's about line

A rule shows its sentence as the client writes rules today; a notice shows
its sender; nothing shows "About your mail".

## Risks

- **Another member's rule or notice.** Both reads answer 404 for what the
  member cannot read or write in, so a chat about them is refused at start
  and goes dark later, as for conversations.
- **List size.** At most 30 chats, kept 30 days.
