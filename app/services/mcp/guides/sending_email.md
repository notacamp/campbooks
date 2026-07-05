# Sending email

## Safety rule — read this first

**Always show the user the exact text and get their explicit approval before
calling send_email, reply_email, or forward_email.** Never invent recipients.
Never send without confirmation, even when the user says "send it."

## Discovering sending accounts

list_email_accounts returns all connected accounts with their `can_send` flag.
Use the `id` as `email_account_id` when calling send_email or
create_scheduled_email. If `can_send` is false for an account, sending from
it will return a ToolError.

## Send a new email

send_email(email_account_id, to_address, subject, body) — body can be HTML or
plain text. `to_address` accepts comma-separated addresses. cc_address and
bcc_address are optional.

Show the full composed email in the chat (from, to, subject, body) and wait for
explicit user confirmation before calling this tool.

## Reply to an email

reply_email(id, body) replies to the given email. The reply threads from the
source message and sends from its originating account unless `email_account_id`
is overridden. `to_address` defaults to the original sender.

When drafting a reply: write the draft in the chat, label it as a draft, ask
"Shall I send this?", and only call reply_email after an explicit yes.

## Forward an email

forward_email(id, to_address) forwards the email to a new recipient. Subject
is preserved. Ask the user to confirm the recipient before forwarding.

## Scheduled emails

Scheduled emails send at a future time. They support RRULE for recurring sends
(e.g. weekly team updates).

- create_scheduled_email — like send_email but with `scheduled_at` (ISO-8601)
  and optional `rrule`.
- list_scheduled_emails — shows pending and upcoming scheduled sends.
- update_scheduled_email — edits a pending send (changes to sent/cancelled ones
  return a ToolError).
- cancel_scheduled_email — soft-cancels by setting status to cancelled.

The email_scheduling entitlement must be active on the workspace plan for
create/update/cancel to work.

## Account recommendations

- If the user has multiple accounts (work + personal), ask which one to send from
  before composing. Do not guess.
- Never send from an account the user has not explicitly chosen or that
  `list_email_accounts` shows with `can_send: false`.
