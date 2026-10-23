# Decisions needed

Open questions that need a human. One heading per decision, each with the
options and a recommended answer. Remove an entry once it is decided and
recorded (ADR, spec or issue).

## Default `complete` model (drafting, summaries, chat)

The `decide` role is settled (Laya, ADR 0009). The default open-weight model
for `complete` — drafting replies, summarising, the assistant's conversation,
schema-guided extraction — is not. Candidates in the catalog today: Qwen3 8B,
Mistral Small 3.2 24B, Gemma 3 12B, gpt-oss 20B, Llama 3.1 8B. Any of them is
configurable per instance and per workspace regardless of the default.
**Recommended:** let the AI-gateway eval set (drafting quality in en/pt/es/fr,
extraction accuracy, latency and memory on the reference hardware) pick the
default, and decide the hosted-cloud model together with the GPU sizing issue.

## Model endpoints for decide evals (Laya + an open-weight instruct model)

`cargo xtask eval decide` (campbooks-cloud#59) compares the triage strategies
per question, but only the offline ones (email signals, lexical k-NN, their
cascade) have been run: there is no Laya, instruct-model or embedding
endpoint available to the agents, and no local Docker. Laya and the instruct
model are implemented and tested against mocks and report "not run". The ADR
amending 0009 with the strategy per question waits for those numbers.
**Recommended:** a small hosted open-weight endpoint (an instruct model with
logprobs, e.g. one of the catalog's 7–12B models, plus an embedding model),
or the staging host's `ai` compose profile for Laya, reachable by a scheduled
CI job with `CAMPBOOKS_EVAL_*` secrets. Run
`cargo xtask eval decide` once there, then write the ADR from the report.

## Labels made in Campbooks: create them in Gmail?

Write-back (campbooks-cloud#66) mirrors Gmail labels a member adds or takes
off, but a label created in Campbooks has no Gmail label yet, and the
`changes_requested` intent carries only the label's id (its name is personal
data). Today such a change is recorded as `skipped`
(`label_not_at_provider`). Options: (a) keep Campbooks labels in Campbooks;
(b) create the Gmail label on first use, with conversations adding an
optional label-name blob to `add_label` (an additive change to the v1
contract) and the connector calling `labels.create` (5 units), after which
conversations links the two by name as it already does; (c) ask the member
the first time. **Recommended:** (b), with a per-inbox switch defaulting to
on, since members expect a label they add to show on their phone.

## Bringing back an expired ask

An ask whose lapse time has passed (a link or a code expired, an offer
closed) is cleared as `lapsed` (campbooks-cloud#275). The lapse time comes
from Scout's read of the email, so it can be wrong, and the member cannot
put the ask back on Home today: the conversation only shows "Expired".
Options: (a) leave it (the member still has the conversation and can reply
or act); (b) offer "Still to do" next to the Expired chip, which makes it the
member's own need, cleared only by a member; (c) also let a member correct
the lapse time. **Recommended:** (b), with the member's need then never
lapsing; it keeps "a member's verdict wins" and needs no new data.

## Calendar: unopened bookings without a need

With one Jev pass and at most one chat read per email (`add-schedule-module`,
campbooks-cloud#42), the read runs only for flagged mail that raised a need
or that a member opened. A booking that raises no need (a flight, a hotel, a
concert ticket) and is never opened reaches the calendar only when it
carries an iCalendar part or schema.org markup (ADR 0021). Options: (a)
accept it; (b) also read flagged `has_dated_thing` mail on arrival (more
chat calls, the cost the owner ruled out); (c) a per-workspace switch for
(b). **Recommended:** (a). Most large booking senders ship markup or an
`.ics`, so those land anyway. Measure on staging how many flagged,
unmarked, unopened bookings are missed before revisiting.

**Decided 2026-10-23:** the recommendation, (a). The owner approved the
prototype and said to implement it.

## Calendar: no summary on arrival for a plain Reply need

Today `summarize_need` reads the email behind every need. The message read
replaces it and runs only for flagged mail (`asks_action`, `has_deadline`,
`has_dated_thing`), so a plain Reply need (a person's question with no ask,
deadline or date) gets no read on arrival. Its summary comes on first open,
and until then Home shows the need's line from the decide pass. Options: (a)
accept it; (b) keep reading every need's email on arrival. **Recommended:**
(a). It costs fewer calls than today, and a Reply need's email is usually
short enough to open.

**Decided 2026-10-23:** the recommendation, (a). The owner approved the
prototype and said to implement it.

## Calendar: shared inboxes add no Scout events

An item read from mail in a shared inbox has no single owner whose calendar
it belongs on. Options: (a) add nothing; (b) add it to the calendar of the
first member who opens the message; (c) a shared Campbooks calendar per
shared inbox. **Recommended:** (a) in this change. Revisit with calendar
sharing (reader/writer/manager), which is out of scope here.

**Decided 2026-10-23:** the recommendation, (a). The owner approved the
prototype and said to implement it.

## Calendar: machine-readable dates may assert (ADR 0021)

ADR 0018 lets deterministic code only say "no". ADR 0021 (Proposed) lets
iCalendar parts and schema.org markup from DMARC-aligned senders assert
dated items and asks: typed fields only, a proper parser, never the prose
around them. Options: (a) accept the carve-out; (b) markup may only veto, and
the model reads everything (one chat call per booking). **Recommended:**
(a). It is exact, free, and how mail clients already read bookings.

**Decided 2026-10-23:** the recommendation, (a). The owner approved the
prototype and said to implement it.

## Calendar: the flag threshold is unmeasured

`has_dated_thing` (and the flag on `asks_action` / `has_deadline`) gates the
chat read at `CAMPBOOKS_CONVERSATIONS__DATED__FLAG`, default 0.6. That is a
starting point, not an operating point from the eval report. No labelled
dated-thing items exist, and items written by the agent that builds the
feature would not count (#447). Options: (a) ship 0.6 and record every
answer, then set the default from owner-labelled items plus members' Undo /
Remove; (b) wait for a labelled set before shipping. **Recommended:** (a),
with 30–50 owner-labelled emails (bookings, bills, renewals, newsletters
with dates) added to the decide eval before the default is final.

**Decided 2026-10-23:** the recommendation, (a). The owner approved the
prototype and said to implement it.

## Calendar: Google calendar scopes and verification

Calendar access asks Google for `calendar.events` and
`calendar.calendarlist.readonly`. Both are *sensitive* scopes (not
restricted), so they join the OAuth verification already tracked in
campbooks-cloud#53. Until that is approved, more than 100 users see the
unverified-app screen. Options: (a) add them to the #53 submission now; (b)
ship calendar to the cloud only after verification. **Recommended:** (a).
Self-hosters are unaffected.

**Decided 2026-10-23:** the recommendation, (a). The owner approved the
prototype and said to implement it.

