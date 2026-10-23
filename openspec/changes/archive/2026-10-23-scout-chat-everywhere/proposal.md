## Why

campbooks-cloud#305: "when i have an interaction with the ai agent it
should be possible to continue it over a chat". Phases 1 and 2
(`continue-with-scout`, `scout-proposes-actions`) opened the chat from a
Home need and the reader. Scout also speaks in other places: it notices
mail an inbox gets a lot of, compiles taught sentences into rules, drafts
replies and Nudges, and lists what the member waits on. Each of those is an
interaction the member may want to continue. A chat once closed is also
hard to find again, and there is no way to ask Scout something not about
one record.

## What changes

- A chat may also be about a rule (`conversations.rule`, read through
  `conversations.get_rule`), about one of Scout's noticed suggestions
  (`conversations.rule_notice`, read through a new
  `conversations.get_rule_notice`), or about nothing (asked from ⌘K).
- "Ask Scout" where Scout spoke: Scout noticed (on Home), a rule after Teach
  Scout saves it and in the rules list, the Reply and Nudge dialogs (about
  their conversation), and Waiting on others rows.
- `assistant.list_chats`: the member's chats, newest first; the panel shows
  them under "Recent" and reopens one.
- ⌘K: "Ask Scout" opens a new chat about nothing, with what was typed as its
  first question.
- Starter questions per kind of record.

## Non-goals

- The activity page (campbooks-cloud#89) is not built yet: a chat's steps
  are already stored per answer, for that page to read when it exists.
- Calendar events and tasks register their own record types and buttons in
  their own changes.
- Searching inside chats.

## Capabilities

### Modified capabilities

- `assistant-chat`: about types (rule, rule notice, nothing), `list_chats`.
- `assistant-chat-ui`: entry points, Recent, ⌘K, starter questions per type.
- `conversations`: `conversations.get_rule_notice`.

## Modules

`assistant`, `conversations` (one read operation), and the client modules
that host the buttons (`conversations`, `attention`, `assistant`), through
the `lib/` opener.
