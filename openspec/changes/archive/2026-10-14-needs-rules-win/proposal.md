# Proposal

Module: `conversations` (needs), and the email signals in `campbooks-ai`.
Tracking: https://github.com/notacamp/campbooks-cloud/issues/123 (attention
follow-ups after campbooks-cloud#39).

## Why

In production, a code host's notification was put to its owner as a
question: "Did GitHub ask you to do something? Campbooks is not sure." The
rules had answered every question about it with confidence and acted: it
asks nothing (`signals.list_mail`, 0.97), needs no reply
(`signals.service_sender`, 0.99), names no date (0.95). The model, which has
not been measured and so never acts, said it asks for an action (0.875). The
need's `raise` puts a confident model "yes" to the member whenever no `sure`
need exists, without looking at whether an answer that acted already said
"no". So the most certain answer lost to the least certain one, and a plain
email became a question.

The mail itself was a security alert about the owner's own repository. The
owner's decision: such an alert *is* something to do, a sure Do, not a
question and not hidden. The rules had no signal for it.

## What Changes

- **A settled answer is not questioned.** A model suggestion becomes a
  `check` question only when no answer the policy let act says "no" to that
  question. When several answers to one question acted, the rules' answer
  wins over the model's (for the need's reason and for its due date).
- **A need its message no longer raises is withdrawn.** When the
  conversation's need is refreshed and its own message, read again, raises
  nothing, the open need is cleared as `superseded` (Home drops it; it is not
  shown under Done today), unless a member confirmed it. This removes needs
  raised before this fix whenever the conversation is refreshed; the needs
  backfill (the next change) refreshes every recent conversation and every
  open need on upgrade.
- **Security alerts are a to-do.** A new rules signal,
  `signals.security_alert`, answers `asks_action` true for a service's
  notification whose subject says something of the recipient's needs fixing
  ("security alert", "security advisory", "vulnerability", "action
  required", in the four languages), and sorts it to Updates. It applies
  only to a service sender (list mail or an automated address) whose display
  name names its own domain ("GitHub" at `github.com`), with no phishing tell
  and no marketing words, and it outranks `signals.list_mail`'s "no".
- Two fixtures: a Dependabot-style security alert and a comment
  notification from the same code host. The reason's sentence is shown in
  the reader in all four languages.

## Non-goals

- Changing the model's prompt or envelope. The model already receives the
  bulk headers (`List-Id`, `List-Unsubscribe`, `Precedence`,
  `Auto-Submitted`) raw, but not the rules' reading of them; whether to pass
  it is noted in the design, not done here.
- The Home card's "concrete next step" (the alert's own link as the main
  action): built separately.
- Re-measuring the policy: the new rule fires on none of the 320 labelled
  items of `mail-triage` v1, so the offline eval is unchanged; its confidence
  is a prior until the set gains security alerts.
- The backfill of needs for mail stored before attention (next change).
