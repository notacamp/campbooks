# Automation

Campbooks supports three automation mechanisms over MCP: workflows, scheduled
emails, and email templates.

## Workflows (feature-gated)

Workflows are workspace-scoped automations with a trigger and an ordered list
of steps. They are available when the workspace has the workflows feature enabled
(get_setup_status.features.workflows = true).

### Types

- **email_received** — fires when a new email arrives and passes processing.
  The trigger context exposes the email and its extracted documents to Liquid
  templates in each step.
- **webhook** — fires when an external service POSTs to the workspace's unique
  webhook URL, or when you call trigger_workflow.

### Calling over MCP

- list_workflows — lists enabled and disabled workflows.
- trigger_workflow(id, payload) — triggers an enabled webhook workflow with an
  optional JSON payload. The payload is exposed to the workflow's Liquid
  templates as `payload.*`. Only webhook-type workflows can be triggered this
  way; email_received workflows fire automatically.
- list_workflow_executions(workflow_id) — shows the run history with each step's
  input/output for debugging.

Only enabled workflows can be triggered. Calling trigger_workflow on a disabled
workflow returns a ToolError.

## Scheduled emails

See the sending_email guide for full details. The key automation use case is
recurring sends — pass an iCal RRULE string to create_scheduled_email to set
up weekly reports, monthly reminders, or any fixed cadence.

Example RRULE values:
- Weekly on Monday: `FREQ=WEEKLY;BYDAY=MO`
- Monthly on the 1st: `FREQ=MONTHLY;BYMONTHDAY=1`

list_scheduled_emails shows the `next_occurrence_at` field so you can confirm
the cadence is correct before telling the user.

## Email templates

Email templates are reusable message templates stored in the workspace. They are
visible when the email_templates feature flag is on
(get_setup_status.features.email_templates = true).

list_email_templates returns the available templates. To use a template when
composing, retrieve its body and subject, present them to the user for any
personalisation, then pass the result to send_email or create_scheduled_email.

## Combining automation patterns

A common pattern: a workflow fires on email_received when the email matches a
certain category, and your agent is separately scheduled (or triggered by a
webhook) to do a morning triage via get_skim_deck + skim_decide. These two
mechanisms are independent — workflows run server-side without agent involvement;
the skim/triage loop runs when you explicitly call those tools.
