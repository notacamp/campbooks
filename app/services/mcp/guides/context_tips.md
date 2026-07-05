# Context tips — keeping your agent lean

MCP tool responses can grow large. These patterns keep context tight without
losing usefulness.

## Use counts before bodies

get_overview returns counts without fetching any content. Check unread_count,
needs_review_count, and skim_pending_count first. Only fetch bodies (get_email)
when the user asks to see a specific message or when the count signals
something needs immediate attention.

## Default to plain text, not HTML

get_email returns plain text by default (format not passed). The text is
converted from HTML and truncated at 20 000 chars (quoted history included). This is much smaller
than raw HTML. Pass format="html" only when you actually need the markup —
for example, to preserve a formatted table the user wants to inspect.

## Limit list results

Every list tool accepts a `limit` parameter (default 20, max 50). Pass the
smallest limit that serves the task. For a summary ("how many unread?") you
do not need list results at all — get_overview has the count.

## Batch ids into bulk tools

update_emails, tag_emails, and move_emails_to_folder accept up to 100 ids each.
Batch related emails into one call rather than looping per message.

## Use search_emails, not list_emails, for targeted lookups

list_emails returns the most recent N emails. search_emails accepts a text
query, sender, category, date range, and attachment filter. Use it when the
user asks for something specific — it is faster and returns exactly the
matching messages rather than a slice of the inbox.

## Scope-narrow clients see fewer tools

tools/list is filtered to your token's scopes. An agent that only reads email
should have only emails:read (plus meta, which is always visible). A narrower
scope set means fewer tools in the list, less context in the system prompt,
and a smaller surface for mistakes.

## Load guides on demand, not at startup

guide() content is returned on demand — do not pre-fetch all guides. Call
guide(topic) when you are about to do something in that area (guide("triage_and_skim")
before a skim session). Topics are listed in the guide() index call (no args).

## Prefer ids over re-fetching

When you have an id from one call (e.g. a document_id from get_email linked.document_ids),
use it directly in the next call (get_document(id)) instead of searching again.
Store ids in your working context rather than re-running searches.

## Use get_skim_deck clusters, not individual emails

The skim deck provides summaries and importance scores at the cluster level.
You rarely need to read every email in a cluster — the summary is usually
sufficient to propose a decision. Fetch get_email only when the user asks
"what does this specific email say?"

## get_setup_status is diagnostic, not conversational

get_setup_status is meant for onboarding flows and troubleshooting. Do not
call it on every session start — use get_overview instead. Call
get_setup_status when the user says something is not working or when setting
up a new workspace.
