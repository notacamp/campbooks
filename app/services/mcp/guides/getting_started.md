# Getting started with Campbooks over MCP

Campbooks is an AI-native email client that sorts itself. Over MCP you have
access to the same operations the web UI exposes — triage, search, documents,
tasks, calendar, and sending — through plain tool calls.

## Recommended first calls

1. **get_overview** — a cheap snapshot: unread count, documents needing review,
   tasks due today, and upcoming calendar events. Call this before anything else;
   it respects your token scopes so only the sections you can see come back.

2. **guide(topic)** — call with no args for the list of topics, then fetch the
   guide relevant to your current task. Guides are loaded on demand and do not
   cost round-trips to the provider.

3. Act on what the overview surfaced. The tool families map to the sections
   get_overview returns: emails → search_emails / get_email / update_emails /
   skim tools; documents → get_document / approve_document; tasks → list_tasks /
   create_task; calendar → list_calendar_events / create_calendar_event.

## Tool families

| Family | Key tools | Scope needed |
|---|---|---|
| Meta | get_overview, get_setup_status, guide | (any authenticated client) |
| Email — read | list_emails, search_emails, get_email | emails:read |
| Email — act | update_emails, move_emails_to_folder, tag_emails, forward_email | emails:write / emails:send |
| Skim triage | get_skim_deck, skim_decide | emails:read / emails:write |
| Email accounts | list_email_accounts, connect_email_account | email_accounts:read / write |
| Documents | list_documents, get_document, approve_document | documents:read / write |
| Tasks | list_tasks, create_task, complete_task | tasks:read / write |
| Calendar | list_calendars, list_calendar_events, create_calendar_event | calendar:read / write |
| Reminders | list_reminders, confirm_reminder | reminders:read / write |
| Scout chat | list_scout_threads, send_scout_message | scout:read / write |
| Taxonomy | create_tag, create_document_type, create_folder | tags/document_types/folders scopes |

## Scopes model

Your API client is issued a set of Doorkeeper scopes. tools/list returns only
the tools those scopes cover. get_overview and guide are always visible (they
require no scope). Narrow clients produce shorter tool lists, which reduces the
context your agent carries — this is a feature, not a limitation.

## Output philosophy

All tools return compact JSON. Lists use `{ <plural>: [...], count: n }`.
Timestamps are ISO-8601. get_email returns plain text by default (truncated to
20 000 chars); pass format="html" only when you need the raw markup.
The skim deck drops the full email array — use get_email(id) for the body.

## Safety rule

Always show the user the exact text and get their approval before calling
send_email, reply_email, forward_email, or any destructive bulk action
(update_emails action=trash). Never invent recipients. Never act in the
background on behalf of the user without confirmation.
