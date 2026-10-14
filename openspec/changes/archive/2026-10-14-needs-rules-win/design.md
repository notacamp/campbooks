# Design

## Data ownership and events

Nothing new is stored. `conversations.needs` (schema `conversations`) keeps
its columns; a withdrawn need is written as `state = 'cleared'`,
`cleared_because = 'superseded'`, and `conversations.need_changed` v1 is
published in the same transaction exactly as for any other clear (null need,
cause `superseded`). `attention.project_conversation_need` already clears the
item on it, and `attention.list` already leaves `superseded` items out of
Done today. No endpoint, scope or contract changes.

## AI use

No new AI call. The `decide` role's answers are read as before; this change
only stops a model answer that the policy did not accept from overriding an
answer it did. Without a `decide` role nothing changes (rules only, no
`check` needs). Undo: Done / Not needed / Reopen are unchanged.

## 1. Which answer wins

`raise` already used only accepted answers for `sure` needs. The gap was the
fallback: "a model yes with confidence ≥ 0.8, not accepted" became a
question even when the rules had acted on "no".

| Option | Behaviour | Trade-off |
| --- | --- | --- |
| A. Acted answers settle their question (chosen) | A model suggestion is put as a question only when no acted answer to that question says "no"; among acted answers to one question, the rules' wins | One rule, mirrors what "acted" means (the policy trusts it); a model can still raise a question where the rules were unsure |
| B. Raise the check floor (e.g. 0.95) | Fewer questions overall | Would still question the prod case at 0.976 for `has_deadline`-like answers, and hides genuine questions where the rules said nothing |
| C. Drop model questions entirely | No `check` needs | Loses the one place the unmeasured model is useful: mail the rules could not read |

A wins: the policy is the source of truth for what may act, and an acted
answer is by definition one we stand behind. It is not a pattern from the
Rails app (which had no policy table).

`accepted` now takes the acted answers to a question, prefers the rules',
and returns it only when it says yes. In the shipped baseline model answers
never act, so this matters only once a model is measured; it keeps an acted
rules "no date" from being overridden by an acted model "Friday".

## 2. Needs raised before the fix

`refresh` re-read the newest message's answers but kept the stored open need
when `raise` returned nothing, so a question raised under the old rule would
stay until a member answered it.

| Option | Trade-off |
| --- | --- |
| A. Withdraw on refresh (chosen) | When the stored need's own message, read again, raises nothing, clear it as `superseded`; a member's confirmation keeps it. No new cause, contract or copy. Happens on any refresh, and the needs backfill refreshes every open need on upgrade |
| B. A one-off job re-evaluating open `check` needs | Same effect for existing rows, but a second mechanism next to the backfill, and future rule changes would need another |
| C. A new cause `withdrawn` | More precise in the event, but a contract (`need_changed` v1 enum) and client change for a case members never need to tell apart from `superseded` |

Only the need's *own* message is re-read this way: a newer message that
raises nothing still leaves an older message's need open (a "thanks!" does
not clear a request), as before.

## 3. Security alerts

The owner decided that a service telling you something of yours needs
fixing (a vulnerability in your repository's dependencies, an advisory on
your account) is a to-do with a next step, while other notifications from
the same service ask nothing.

| Option | Trade-off |
| --- | --- |
| A. A rules signal (chosen) | Deterministic, explainable ("It is a security alert about something of yours"), no AI; narrow guardrails keep phishing and marketing out |
| B. Let the model decide for list mail | The model is unmeasured and never acts under the policy; it would also bring back questions for every notification |
| C. A per-sender allowlist (github.com, gitlab.com…) | Precise for known hosts, but a list to maintain and nothing for other services; the generic shape covers them |

Guardrails, all required:

- **Service sender:** list mail or an automated local part (the signals'
  existing notions).
- **The sender names its own domain:** a word of the display name, or the
  whole name, equals the first label of the sender's registrable domain,
  letters and digits only ("GitHub" at `github.com`; not "GitHub" at
  `github-alerts.example`, not "Security Team" at `github.com`). This is the
  "domain matches the platform it claims" check.
- **No phishing tell** (look-alike domain, reply-to elsewhere, phishing
  phrases): a phishing "security alert" stays a warning, and the needs'
  suspicious check runs first anyway.
- **No marketing words** in the subject or text: "Action required: claim
  your discount" stays a newsletter.
- **Subject only:** a newsletter article about vulnerabilities is not an
  alert.

It answers `asks_action` true (`signals.security_alert`, ahead of
`signals.list_mail`), and `stream` Updates with the same confidence. Its
confidence (0.9) is a prior: `mail-triage` v1 has no security alerts, the
offline eval is unchanged (the rule fires on none of its 320 items), and
labelling some is the follow-up that measures it.

## What the model sees for such mail

`Envelope::decision_state` gives the model From, To/Cc, Subject, the bulk
headers present (`list-unsubscribe`, `list-id`, `precedence`,
`auto-submitted`, `reply-to`) truncated to 120 characters, attachment names
and the first 1,500 body characters. So it does see that a GitHub alert is
list mail, but not the rules' reading ("a service sent it", "sent to a
list", "a security alert"). Passing the rules' verdicts as context could make
the model echo the rules rather than add an independent opinion, which
defeats the cascade's purpose and would bias the eval; adding a short neutral
"sender kind" line (person, service, list) is the one piece worth measuring.
Not changed here.
