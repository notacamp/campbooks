# Personal-data breach response runbook — Campbooks

> ⚠️ Review-ready draft, not legal advice (Art. 33 / 34). Adopt, fill the
> **[TODO]** owners/contacts, and rehearse it.

A "personal-data breach" = any breach of security leading to accidental or
unlawful **destruction, loss, alteration, unauthorised disclosure of, or access
to** personal data. The **72-hour clock to notify the supervisory authority
starts when you become aware** a breach has (likely) occurred — not when you
finish investigating.

## Roles
| Role | Who | Responsibility |
|---|---|---|
| Incident lead | [TODO — incident lead name] | Owns the response, makes the notification call |
| Technical responder | [TODO] | Containment, forensics, log review |
| Privacy/legal | [TODO counsel] | Art. 33/34 assessment, CNPD liaison |
| Comms | [TODO] | Data-subject + customer messaging |

**Supervisory authority:** CNPD, Portugal — https://www.cnpd.pt — [TODO breach-notification channel/form].

## Step 1 — Detect & record (immediately)
Trigger sources: GlitchTip alerts, abnormal access, a report, a leaked credential,
a sub-processor notifying us. **Start an incident log now** (UTC timestamps): what,
when discovered, who, systems involved. Record the **time of awareness** — it anchors the 72h clock.

## Step 2 — Contain
- Revoke/rotate exposed credentials (`ACTIVE_RECORD_*` keys, OAuth secrets, API keys, `master.key`).
- Invalidate sessions if relevant (`Session.delete_all` for affected users / force re-auth).
- Isolate the affected component; preserve evidence (don't wipe logs).
- If a sub-processor is the source, get their breach report.

## Step 3 — Assess severity & scope
Determine: data categories, **number of data subjects**, whether it includes
third-party correspondents / financial PII, and likely consequences. **Note the
weak spot:** there is currently **no access audit log**, so scoping "what was
accessed" may require infrastructure logs — see task list. Classify:

| Severity | Examples | Notification |
|---|---|---|
| **High** | Mailbox/document content or OAuth tokens exposed; financial PII disclosed | CNPD **within 72h** + likely affected data subjects (Art. 34) |
| **Medium** | Limited account data exposed, contained quickly | CNPD likely; document the risk assessment |
| **Low / no risk** | Encrypted data lost but keys safe; near-miss | Usually no notification — **but record why** (Art. 33(5)) |

## Step 4 — Notify (if required)
- **CNPD within 72 hours** of awareness if the breach is likely to result in a risk to individuals. If <72h isn't possible, notify with reasons for delay.
  Include: nature of breach, categories + approximate number of subjects/records, likely consequences, measures taken/proposed, DPO/contact point.
- **Affected individuals without undue delay** if **high risk** to their rights/freedoms (Art. 34) — in clear language, with advice (e.g. reconnect mailbox, watch for phishing).
- Remember third-party data subjects may be affected, not just account holders.

## Step 5 — Remediate & learn
- Close the root cause; verify the fix.
- Update the **internal breach register** (every breach, notified or not — Art. 33(5)).
- Post-mortem → concrete follow-ups (e.g. add audit logging, tighten a permission, rotate on schedule).

## Pre-built containment commands (adapt before use)
- Rotate keys: see your ops runbook / secrets store for the deploy/secrets process.
- Force-logout everyone: `Session.delete_all` (all users re-authenticate).
- Disable a compromised email/calendar account: `account.deactivate!` (and revoke its grant — `oauth_client.revoke_token`).

Last updated: 2026-06-22.
