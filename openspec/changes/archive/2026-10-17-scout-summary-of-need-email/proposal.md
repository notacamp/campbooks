# Scout's summary of the email on an opened Home item

## Why
The owner asked for "at least an AI summary" so a decision on a Home item does not need the conversation opened. #203 shows the email a few lines deep; a long email still has to be read to know what it is about. The approved prototype (v6) shows Scout's summary first, under a small heading with the visor, above the email.

Module: `conversations` (server), `attention` and `lib/ui` (client). Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/204.

## What changes
- When a need opens for a message, a job (`conversations.summarize_need`) asks the `complete` role for one or two plain sentences saying what the email is about, in English, European Portuguese, Spanish from Spain and French in one call. The result is stored once with the message (`conversations.need_summaries`) and never written again; opening an item never asks the model.
- The summary is background work: it counts against the workspace's daily AI limit, one call per message that raises a need, and runs only while the `complete` role may receive mail on this server.
- `conversations.get` returns `need_summary`, the open need's summary in the caller's language (English when theirs is missing), or none. A new event `conversations.need_summarised` (ids only) refreshes an open Home when a summary lands.
- The opened Home item shows it first, under "Scout's summary" with the visor, above the email. Nothing is drawn when there is no summary, no role, or the call failed; the email still shows.
- On upgrade every inbox is queued for the needs backfill, and its page job queues a summary for each open need that has none, with no manual step.

## Non-goals
- A summary in the inbox list, the reader or notifications.
- Summarising whole conversations, attachments or non-need mail.
- Regenerating or editing a summary; asking the model when an item opens.
- Putting any preference or rule of the member into the prompt.

## Impact
- Server: one table, one job, one event, one field on `conversations.get`. No new scope or endpoint.
- Client: `ItemDetail` `summary` slot, Home's `NeedDetail`, strings in four languages.
