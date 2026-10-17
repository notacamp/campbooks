# Design: the action catalogue

## Where the questions are asked

| Option | Cost | Triage risk | Chosen |
| --- | --- | --- | --- |
| **Same `decide` call as triage, extra `noul` questions** | only the questions' own tokens (the email's state is sent once) | each question is its own slot (`questions` is a map keyed by id): triage goes out byte-for-byte the same, a test pins it | **yes** |
| A second call per need | the whole state again per item | none | no: doubles the input cost |
| Asking per action at render time | a call per Home load | none | no: latency and cost on every load |

The money answer is not known when the questions are asked (same call), so
`action_paid_automatically` is asked whenever `money` is, which is always,
and only read for a Pay need.

## Reusing triage answers

Three actions already have their likelihood in a triage answer, so they cost
nothing: Reply (`needs_reply`), Report and It's safe (`suspicious` and its
complement). Already paid and Still to pay share one question. Four new
questions in all; three for mail without an attachment.

## The facts only rule out

| Action | Ruled out when |
| --- | --- |
| Reply | the sender is a no-reply address (`EmailFacts::no_reply_sender`) |
| Open attachment | the message has no attachment (the question is not even asked) |
| others | never |

## Storage

`conversations.action_likelihoods` (message, action, likelihood, model),
row-level security, cascading with the message. Not `decisions`: that table
feeds How decided, the needs and the backlog's "answered" count, and the
catalogue must change none of them.

## Reading, and the kill switch

`conversations.list` and `conversations.get` (what Home reads) attach the
actions, and only while the flag is on: the handlers take the settings as a
router extension, so the floor applies when read (a new floor applies at
once) and switching the flag off hides every rating at once, even those
stored while it was on. Summaries returned by actions (a need resolved, a
star) carry none; Home refetches its rows after an action. Mail decided
before the switch-on is not rated until a new message arrives in its
conversation.

## Ranking

| Need | Its own answers (always, likelier first) | Others, if above the floor |
| --- | --- | --- |
| Review (suspicious) | Report, It's safe | none |
| Pay | Already paid, Still to pay | Open attachment, Remind, Forward |
| Reply, Do, other Review questions | — | Reply, Open attachment, Remind, Forward |

At most three in all (`MAX_ACTIONS`). A need with nothing rated, or nothing
cleared, carries no `actions` and the client offers its fixed actions.

## Combos

`COMBOS` is one small table. The approved prototype's only pair is "Say yes
and add to calendar" (Reply with Add to calendar); Add to calendar joins with
#42, so the table starts empty. A pair is offered as one, in first place
among the others, when both actions cleared the floor; its likelihood is the
smaller of the two. `cargo xtask eval actions` reports, from the owner's
labels, how often each pair is done together, so the table is pruned or
extended on evidence.

## Off is byte-identical

With the flag off the pass asks exactly `triage_questions()` (tested on the
request body), stores no rating, and `NeedView` serialises without an
`actions` key (`skip_serializing_if`, tested on the JSON). With it on, the
triage answers stored and acted on are the same (tested against the off run).

## The floor comes from the owner

The floor is never tuned on emails an agent wrote (ADR 0018).
`labelling export --set actions` writes the owner's mail that had a need
(the same guards as the money export: operator-only CLI, a new folder
readable by its owner only, real mail never committed); the owner marks
`yes` under each action they would take. `cargo xtask eval actions` asks
the model (Jev by default), rates exactly as the pass does, and prints
precision and recall per action at each floor.

## Cost

Per email, on top of triage: the four questions' text (about 480 characters,
some 130 tokens with the ids and the protocol's framing; three questions,
about 95 tokens, without an attachment) and four short answers. See the PR
for the per-1,000 estimate. No call is added.

## max_tokens

The OpenAI-compatible decide path sent no `max_tokens`, so a server default
(often a few hundred tokens) could cut a reasoning model off before its
JSON. It now sends 4096. It is a cap: the answer is a short JSON object, so
neither cost nor latency changes for a model that answered before; a
reasoning model that was cut off now answers. Jev (the Laya protocol) has no
such field.
