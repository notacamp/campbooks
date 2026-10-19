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
