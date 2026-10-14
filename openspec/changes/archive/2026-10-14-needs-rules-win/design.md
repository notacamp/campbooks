# Design

## Data ownership and events

Spam and trash: a need comes only from the newest inbound message in neither
(`messages.in_spam`, `messages.in_trash`, which Gmail's SPAM/TRASH labels
and Zoho's spam and trash folders already set), and a need whose own message
moves there is cleared as `trashed`.

`connectors` adds one optional field to the message change it publishes
(`headers.authentication_results`, `MessageChangeBatch` v1, backward
compatible); `conversations` stores it with the message's other headers
(`messages.headers`, jsonb). Nothing else new is stored. `conversations.needs` (schema `conversations`) keeps
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
| A. A rules signal on authenticated mail, acting only for listed platforms (chosen) | Deterministic, explainable ("It is a security alert about something of yours"), no AI; a short list to keep, and other services' alerts are at most a question |
| B. Let the model decide for list mail | The model is unmeasured and never acts under the policy; it would also bring back questions for every notification |
| C. The generic shape alone, any sender naming its own domain | Covers every service, but a look-alike domain that signs its own mail passes it: rejected by the security review |

Guardrails, all required (a security review of the first version showed
that headers a sender controls are not enough: look-alike domains with
their own DKIM, a spoofed From through a provider that does not enforce
DMARC, a Cyrillic domain, and a bare "action required" all became a sure Do):

- **Authenticated by the receiving provider.** The connector keeps the
  topmost `Authentication-Results` only when its authserv-id is the
  receiving provider's (Gmail `mx.google.com`, Zoho's regional exchangers;
  anything else, or an unknown id, is no result). The rule answers only on
  `dmarc=pass` for the From domain, or `dkim=pass` for a signing domain with
  the same registrable domain. Absent or failing results: no answer.
- **A sure Do only for listed platforms.** `github.com`, `gitlab.com`,
  `bitbucket.org`, `npmjs.com`, `pypi.org`, `rubygems.org`, `crates.io`,
  `snyk.io`: code hosts and package registries, whose alerts are about the
  recipient's code. Any other authenticated sender gets 0.6, below every
  operating point the baseline acts from (0.75 and up): at most a question.
  A look-alike that signs its own mail (`githubsecurity.example`) lands
  here.
- **Plain ASCII domains, named by the sender.** A word of the display name,
  or the whole name, equals the first label of the sender's registrable
  domain, letters and digits only; a domain with an `xn--` label or
  non-ASCII characters never qualifies ("Gіthub" with a Cyrillic і).
- **A security word.** "Action required" alone is what phishing says
  (`ACTION_NOW`); it is not in the list.
- **Service sender, no phishing tell, no marketing words, subject only**, as
  before.

The follow-up Home card will offer the alert's own link as the main action;
the spec says that a message's links may be offered that way only when its
sender is authenticated like this and it is not suspicious.

| Option for trusting the sender | Trade-off |
| --- | --- |
| A. The receiving provider's `Authentication-Results`, by authserv-id (chosen) | The provider already checked SPF, DKIM and DMARC; we read one header, keep it only from the receiver, and need no crypto or DNS. Depends on knowing each provider's authserv-id: an unknown one reads as no result (fails closed) |
| B. Verify DKIM ourselves | Independent of the provider, but DNS lookups at sync time, key rotation and a new crate, for what the provider already did |
| C. An allow-list of sender addresses | Simple, but a spoofed From passes it unchanged; nothing proves the mail came from there |

**Reading the header.** The header is the provider's, but it echoes what
the sender chose: the envelope sender in the SPF comment and in
`smtp.mailfrom`, which may be a quoted string holding `;`, `(` or `)`. A
second review showed that splitting on `;` lets
`MAIL FROM:<"a; dmarc=pass header.from=crates.io x"@evil.example>` fake a
pass for a domain that publishes `p=none`. So the header is tokenised as RFC
8601 says (`signals::auth_results`): nested comments dropped, a quoted
string one atom wherever it appears (inside a comment too, so a `)` in it
closes nothing), results split only at a top-level `;`, each result's own
`ptype.property=value` pairs read up to the next one. A malformed header
(unclosed comment or quote, a result that is not `method=value`) reads as no
results. `dmarc` counts only when there is exactly one `dmarc` result, it is
`pass`, and its own single `header.from` is the From domain; `dkim` only
when it is `pass` and every signing domain it names is aligned.

**Topmost, in the message's own order.** The connector walks the raw
headers in order; a topmost `Authentication-Results` that is not UTF-8 makes
the results absent, never the next one down.

**Zoho trusts none yet.** Nobody has seen which header Zoho adds on top of a
received message; if it adds only `ARC-Authentication-Results`, a
sender-written `Authentication-Results: mx.zohomail.com; dmarc=pass …` would
be the topmost one. So Zoho's list is empty (no security alert is a to-do in
a Zoho mailbox) until a recorded Zoho message shows its ids
(campbooks-cloud#132).

**Platform names.** Each listed platform has the display-name words it
signs with (`npm` for `npmjs.com`, `crates.io` for `crates.io`), so its real
alerts are recognised; any other domain must be named by its first label.

**Residual risk: insiders.** Mail between two people of the same Google
Workspace domain may be delivered without a Google-added
`Authentication-Results`. A colleague could then put a forged header on top,
with a spoofed `From: GitHub <noreply@github.com>`, and the alert would read
as authenticated. The risk is limited to people who can already send mail
inside the organisation; reading Gmail's `ARC-Authentication-Results` or the
`Received` chain as well would close it, and is noted as a follow-up.

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

## Confidence

The listed platforms' answer (0.9) and the others' (0.6) are priors:
`mail-triage` v1 has no security alerts, the offline eval is unchanged (the
rule fires on none of its 320 items, which carry no authentication
results), and labelling some is the follow-up that measures them.
