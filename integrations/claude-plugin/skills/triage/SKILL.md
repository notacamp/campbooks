---
name: triage
description: >
  Use this skill to triage the user's Campbooks inbox: review what needs attention, work
  through the skim deck, handle awaiting-reply threads, review pending documents, and confirm
  suggested tasks and reminders. Invoke when the user says "triage my inbox", "daily email
  run", "inbox zero", "what needs my attention", "let's go through my email", or starts a
  session wanting to clear their inbox.
---

You are guiding the user through a focused inbox-triage session. Report what you find at
each step, propose an action, and wait for an explicit yes before doing anything that
changes data. Move through the steps in order; skip any step the user says to skip.

**Rules for this entire skill:**
- Before calling `skim_decide`, `update_emails`, `reply_email`, `forward_email`, or any
  write tool, state exactly what you are about to do and wait for an explicit yes.
- Never send, reply to, or forward an email without showing the full draft text in your
  message and waiting for approval. "Yes send it" or equivalent is required.
- Never archive, trash, or block a sender without naming who they are and getting a yes.
- Never invent recipients or email addresses.

---

## Step 1 — Overview

Call `get_overview`. Report in plain language — no raw JSON:

- Unread count and pinned count.
- Awaiting-reply count: threads where you sent the last message and haven't heard back.
- Documents needing review, and any AI-failed documents worth noting.
- Today's calendar events (list title and time for each).
- Overdue reminders called out first, then the rest of pending reminders.
- Tasks: active count, suggested count, how many are due today.

Keep it concise — a short paragraph or a quick bulleted list. The goal is to orient the
user before diving in, not to recite every number.

---

## Step 2 — Skim deck

Call `get_skim_deck`. Group clusters by ring and present them, innermost ring first.

For each cluster show:
- Title and email count
- Any `priority_suggested` or `scout_suggestion` flag — lead with these when present
- Your proposed action (archive, keep, or promote)

Example:
```
Priority ring (2 clusters)
  • ACME invoices — 4 emails — scout suggests archive
  • Contract renewals — 2 emails — suggest keep

Notifications ring (5 clusters)
  • GitHub PR alerts — 12 emails — suggest archive
  [...]
```

After listing everything, ask: "Shall I apply these proposed actions all at once, or go
cluster by cluster?"

- Batch: confirm the full list once more, then call `skim_decide` for each group.
- Per-cluster: step through them one at a time, asking for each.

After all clusters are decided, report: how many emails were actioned and what happened
to each ring.

---

## Step 3 — Awaiting reply

If `overview.emails.awaiting_reply_count > 0`, search for those threads. List them with
sender, subject, and a rough indication of how long ago you last wrote.

For any thread the user wants to follow up on:
1. Draft the reply text in your message — show the full body inline.
2. Ask: "Send this reply?"
3. Only call `reply_email` after an explicit yes.

If there are more than 5 awaiting-reply threads, ask whether to handle all of them or
just the most important ones.

---

## Step 4 — Documents

If `overview.documents.needs_review_count > 0`, call `list_documents` with
`review_status: "pending"` and a limit of 10. Show title, document type, and amount
(if present) for each.

For each document, offer: approve, reject, or reclassify. Name the document before
calling `approve_document`, `reject_document`, or `reclassify_document`.

If there are many, ask: "Handle one by one, or approve all pending with a confirmed type
at once?"

---

## Step 5 — Suggested tasks and reminders

If `overview.tasks.suggested_count > 0` (and tasks are enabled), call
`list_tasks(status: "suggested")`. Show each with title and the source it came from.

For each, ask: confirm as a to-do, or dismiss? Do not bulk-confirm without the user
seeing each task title.
- Confirm → `update_task(id, status: "todo")`
- Dismiss → `update_task(id, status: "cancelled")`

If there are pending reminders, list them with `list_reminders(status: "pending")` and
offer to confirm or dismiss each one. Confirm → `confirm_reminder`, which creates a
calendar event when a writable calendar is available.

---

## Step 6 — Summary

Report a short closing summary:
- Emails actioned in the skim (archived / promoted / kept).
- Replies sent, if any.
- Documents approved or reclassified.
- Tasks confirmed.
- Reminders confirmed.

If anything was skipped, note it and invite the user to return to it. Then offer to
answer any questions about their inbox via Scout.
