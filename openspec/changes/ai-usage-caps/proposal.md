# Proposal

## Why

Every inbound email triggers a model call (the `decide` pass), and nothing
limits how many. A flood of mail into a connected mailbox, a mailing-list
bomb or a sync loop runs up a hosted provider's bill without bound, and on a
self-hosted CPU model it saturates the host. Nobody can see how much AI a
workspace used today, either.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/152 (split
from https://github.com/notacamp/campbooks-cloud/issues/20).

## What Changes

- **`campbooks-ai`** gets a metering seam (`campbooks_ai::usage`): a gateway
  composed with a `Meter` asks it before every model call (`chat`, `embed`,
  `decide`) and tells it how the call went (tokens when the provider reports
  them, failure). A metered gateway needs to know whose call it is: the
  caller runs it inside a `usage::Scope` naming the workspace and the
  feature. A call outside any scope is refused (`Error::Unattributed`), so a
  new call path cannot skip the meter by accident. Over the cap the call is
  refused (`Error::OverDailyCap`) and nothing is sent. The gateway's own
  answers check runs under an instance scope, which is not capped.
- **`system`** owns the ledger and implements the meter: schema `system`,
  tables `ai_usage_days` (calls admitted and refused per workspace and UTC
  day) and `ai_usage` (calls, failures and tokens per workspace, day, role
  and feature). A call is admitted by one atomic conditional upsert, so
  concurrent workers never go past the cap. The first refusal of a day logs
  one plain line.
- **Cap**: `CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE`, model calls per
  workspace per UTC day. Unset: 5,000. `0`: no cap.
- **`conversations`**: the `decide` pass runs its call in the workspace's
  scope. Over the cap it stores nothing, exactly as when no model is
  configured: the message keeps what the rules decided when it was stored.
  It is not re-queued.
- **Admin API**: `GET /admin/v1/system/ai_usage` (`system.admin_ai_usage`,
  permission `system.read`): a day's usage per workspace (calls, refused,
  remaining, by role and feature with tokens), workspaces that hit the cap
  first. In `openapi/admin.json`.
- **Server**: composes the gateway with the system module's meter and passes
  the same meter to the admin read.

## Non-goals

- **The user-facing notice** ("Today's AI limit was reached; new mail is
  sorted by your rules until tomorrow"). It is UI and needs an approved
  visual prototype first. Follow-up; this change keeps the state in the
  ledger so an API for it is a small read later.
- A per-workspace override of the cap, plans or billing.
- A token or spend budget. Calls are the measure (see design); tokens are
  recorded for visibility. A token budget comes with the first `complete`
  caller, whose output length is not bounded the way `decide`'s is.
- A reserve for interactive, user-initiated AI calls. No such call exists
  yet; the scope is where it will go.
- Re-running the model on mail skipped over the cap, automatically or on a
  person's request (a follow-up with the notice).
- Pruning old ledger rows or dropping them when a workspace is purged (a few
  rows per workspace per day, no content).
- Metering the `xtask eval` tool or the reachability probe: the tool runs
  against a gateway without a meter on an operator's command, and the probe
  (`GET /models`) runs no model.
