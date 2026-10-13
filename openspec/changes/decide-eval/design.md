# Design

## Context

`campbooks-ai` answers typed questions through `Gateway::decide` (ADR 0009),
over Laya or an instruct model, with a temperature per (type, option count).
The conversations module (campbooks-cloud#37) will ask six questions of every
inbound message in one pass: stream (choice of 4), needs a reply, asks the
recipient to do something, has a deadline (noul each), urgency (score 0–2)
and suspicious (noul). Zero-shot Laya got two of three wrong on the first
sample, with near-certain confidence. Before any of these answers acts on
mail, each question needs a measured strategy and a threshold.

## Data ownership and events

- No Postgres schema, table, migration, event or endpoint; no OAuth scope.
- The eval set is repository data owned by `campbooks-ai`
  (`crates/ai/evals/`), versioned with the code that it measures.
- **AI:** roles `decide` (model strategies) and `embed` (k-NN). When a role's
  eval endpoint is unset, that strategy is reported as *not run*. At runtime
  (later, in conversations) a question whose strategy abstains or falls below
  its threshold is asked of the user, never acted on; every automatic action
  stays undoable (AGENTS.md).

## Eval set

```
crates/ai/evals/decide/mail-triage/
  manifest.json        # version, questions, label sets, target precision
  LABELLING.md         # the labelling guide (definitions + edge-case rulings)
  items/en.jsonl  items/pt-PT.jsonl  items/es-ES.jsonl  items/fr.jsonl
  reports/baseline.md  reports/baseline.json
```

One item per line (`message` is a `signals::email::Envelope` plus its date):

```json
{"id": "pt-PT-atelier-01", "lang": "pt-PT", "split": "dev", "workspace": "atelier",
 "hard": ["polite_ask"],
 "message": {
   "from": {"name": "Marta Figueiredo", "address": "marta.figueiredo@example.com"},
   "to": [{"name": "Inês Valadares", "address": "ines@atelierpacheco.example"}],
   "subject": "Cozinha – planta revista", "date": "2026-09-03T09:12:00+01:00",
   "headers": {}, "attachments": [], "body": "Bom dia, Inês, …"},
 "labels": {"stream": "primary", "needs_reply": true, "asks_action": true,
            "has_deadline": false, "urgency": 1, "suspicious": false},
 "rationale": "O pedido é amável ('quando puder') mas …"}
```

- **Workspaces** are four fictional personas (a Lisbon architecture studio, a
  Madrid bakery, a Lyon household, a London freelance consultant), each with
  20 messages in each of the four languages, so k-NN can be simulated per
  workspace.
- **Split** is fixed in the data, exactly half `dev` per stream, workspace and
  language: rule confidences, thresholds and temperatures use `dev` only; the
  report's numbers are on `test`.
- **Mix per language** (80): primary 40 %, updates 20 %, receipts 20 %,
  newsletters 20 %; 10 % suspicious (look-alikes of banks, carriers,
  suppliers and bosses); over two thirds carry a hard-case tag; 35–40 % of
  the automated mail has no bulk headers, because some providers don't expose
  them.
- **Consistency rules** (a `cargo test` fails otherwise): `needs_reply` only in
  `primary` and implies `asks_action`; `urgency > 0` implies `asks_action`;
  `urgency = 2` implies `has_deadline`; suspicious means urgency 0 and no
  reply; unique ids and messages; every address and link on a reserved domain.
- **Every domain is `example.com` or `*.example`.** The first drafts put
  phishing on `.test` and genuine senders on `.example`, which would have let
  a rule read the label off the TLD; all of them were moved to `.example`.
- **Labels are produced to spec, then checked blind.** Each language was
  drafted by one model pass to a target label vector and hard-case quota, then
  labelled again blind, from the guide alone, by another. Agreement before
  adjudication was 97–100 % per question (table in `LABELLING.md`); the 23
  items with a disagreement were settled against the guide and the rulings
  added to it. Both passes are models, so this is an upper bound on human
  agreement. I spot-checked a sample per language.
- **Versioning:** the manifest has a `version`; changing a label or an item
  bumps it, and reports name the version they ran on.

## Strategies

In `campbooks-ai`, the only crate both the harness and the conversations
module may depend on, so the eval measures what ships:

```rust
// decide::strategy
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    /// One entry per question, in order; `None` = abstains.
    fn decide<'a>(&'a self, envelope: &'a Envelope, questions: &'a [Question])
        -> BoxFuture<'a, Result<Decisions, Error>>;
}
pub struct Decisions { pub answers: Vec<Option<Decided>>, pub remote_calls: u32 }
pub struct Decided { pub answer: Answer, pub reasons: Vec<Reason> }
pub struct Reason { pub rule: &'static str, pub evidence: String } // "signals.list_header", "List-Unsubscribe"
```

- `Answer` is the existing `decide` type, so `accept_if(τ)` works the same for
  every strategy. A `Reason` is a stable rule id plus the evidence it matched:
  the UI translates the id for "Why?", logs can print it.
- **`signals`** (`decide::signals::email::EmailSignals`) — the single source of
  deterministic triage rules (the conversations module calls it and keeps no
  copy). Envelope in, an `Option<Decided>` per question out, no I/O. Reads
  bulk headers, `Auto-Submitted`, automated sender local parts, calendar
  invites, attachments, amounts, and cue phrases in en/pt-PT/es-ES/fr
  (requests, polite modals, conditional and "no need" sentences, payment
  terms, paid/due, deadlines in dates and prose, "today", phishing phrases),
  plus phishing tells (look-alike domain shapes, `Reply-To` elsewhere, links
  elsewhere with a payment or login ask, pressure). Abstains where the
  signals don't settle a question. Each rule's confidence is its `dev`
  precision, smoothed as (correct + 1) / (answers + 2). The canonical
  question wording is `triage_questions()`.
- **`knn`** (`decide::knn::Knn`) — embeds `Envelope::embedding_text()`, takes
  the 5 most similar corrections that answer the question, votes weighted by
  similarity with additive smoothing, abstains under 2 neighbours of
  similarity ≥ 0.1. Embedders: `GatewayEmbedder` (the `embed` role) and
  `LexicalEmbedder` (hashed character 3–5-grams, no model, no dependency) — a
  floor, reported under its own name. Examples come through a `Neighbours`
  trait: pgvector in the module later, `InMemoryNeighbours` here. Simulated
  per workspace with its `dev` items as its corrections (leave-one-out on
  `dev`), plus a learning curve with the first 5, 10, 20 and all of them.
- **`model`** (`decide::strategy::Model`) — one `Gateway::decide` call with
  `Envelope::decision_state()` (sender, recipients, subject, the bulk headers
  present, attachments, the first 1,500 characters of the body), Laya or an
  instruct model.
- **`cascade`** (`decide::strategy::Cascade`) — stages in order, each with a
  threshold per question; the first stage whose answer reaches its threshold
  wins, later stages are asked only the open questions, and not at all when
  none is. A failing stage abstains (the earlier answers stand), so a model
  server being down degrades to the rules. In the harness each stage but the
  last uses its own `dev` operating point, and stages replay their recorded
  answers through the real `Cascade`, so a cascade costs no second model call.

## Harness

`cargo xtask eval decide` (xtask already builds `campbooks-ai` through the
server; it gains a direct dependency and a tokio runtime for this command):

```
cargo xtask eval decide                          # every runnable strategy, all questions
cargo xtask eval decide --strategy signals --question stream --lang pt-PT
cargo xtask eval decide --strategy 'signals>knn-lexical>laya'
cargo xtask eval decide --offline --baseline     # rewrite reports/baseline.{md,json}
cargo xtask eval decide --offline --check        # CI: offline strategies vs committed baseline
```

Endpoints come only from eval variables, never the dev `decide` config:
`CAMPBOOKS_EVAL_LAYA_URL` (+ `_API_KEY`, `_MODEL`, default the multilingual
checkpoint), `CAMPBOOKS_EVAL_INSTRUCT_URL` (+ `_API_KEY`, `_MODEL`,
`_LOGPROBS`), `CAMPBOOKS_EVAL_EMBED_URL` (+ `_API_KEY`, `_MODEL`). A missing
variable makes the strategy *not run*, with the reason, in the report.

Per question × language × strategy (and "all languages"):

- **accuracy** (argmax; abstentions counted separately as **answered rate**);
  macro-F1 for `stream`; precision and **recall of the positive class** for
  noul (recall is what matters for `suspicious`); for `urgency` exact
  accuracy, mean absolute error and within-one.
- **coverage@τ and precision@τ** for τ in 0.50–0.99, and the **operating
  point**: the lowest τ whose precision on `dev` reaches the question's target,
  with at least 10 answers covered,
  in the manifest (stream 0.95, needs_reply 0.90, asks_action 0.90,
  has_deadline 0.90, urgency 0.85, suspicious 0.98 for "warn"), then the
  precision and coverage at that τ on `test` with a 95 % Wilson interval. With
  ~40 test items per language the interval is wide; the report shows it rather
  than hiding it.
- **calibration:** ECE (10 bins) and Brier score, raw and after a temperature
  fitted on `dev` (the same `temper` function production uses), and the fitted
  temperature itself — the value to put in `CAMPBOOKS_AI__DECIDE__CALIBRATION__*`.
- **latency:** p50 and p95 per call; and cost proxy = model calls per item
  (what a cascade saves).

- **hard cases:** accuracy per hard-case tag, so "polite asks" or "phishing
  look-alikes" can be followed on their own.

Output: `<out>/report.md` (tables, with whether each operating point *holds*
on `test`), `<out>/report.json` (every number, dataset version, git commit,
date) and `<out>/predictions.jsonl` (every answer with its reasons, for error
analysis; never committed). `--check` re-runs the offline strategies and
fails if any operating-point precision or coverage moves below the committed
baseline by more than 2 points; it runs in CI in about 5 seconds, with no
network.

## Deciding per question

The eval produces, per question, the strategies whose operating point holds on
`test`, ranked by coverage, then cost. The chosen policy
is recorded in an ADR amending 0009 and shipped as a typed default
(`DecidePolicy`: per question, a cascade and a threshold), overridable per
instance. A question with no strategy reaching its target stays
suggestion-only. This change commits the offline baseline; the ADR is written
once at least one model strategy (Laya and/or an instruct model) has been run
on the set, because choosing "rules only" without that comparison would be a
decision by omission.

## Alternatives considered

**Where the eval set lives.**
1. *In the repo next to the code, JSONL* (chosen): reviewed in PRs, versioned
   with the strategies, runnable in CI, no service. Cost: a few hundred KB of
   text in a public repo, so strictly fictional.
2. *A separate dataset repo or a Hugging Face dataset*: decoupled releases and
   reusable for fine-tuning, but versions drift from the code and CI needs a
   network fetch. Worth doing later if the set grows into fine-tuning data.
3. *Only the workspace's own corrections in Postgres*: the real distribution,
   but private, empty until users correct, and impossible to share or run in
   CI. It is the second eval source (same harness, a private export), not the
   first.

**How labels are produced.**
1. *Drafted to a target label vector, then a blind second labelling pass*
   (chosen): cheap, controllable hard-case mix, measured agreement.
2. *Label existing text with an instruct model*: circular when the instruct
   model is a contender, and errors are invisible.
3. *Human-labelled real mail*: the best signal, but it cannot be in a public
   repo; it arrives later as opt-in corrections.

**Where the strategies live.**
1. *Trait and strategies in `campbooks-ai`, the email rules as a documented
   `decide::signals::email` module* (chosen, with the lead's constraint that
   it is the single source of deterministic triage rules: conversations calls
   it and keeps no copy; the API stays small: envelope in, an answer and its
   reasons per question out). The reason: the eval must measure the code that
   ships, and `campbooks-ai` is the only crate both the harness and the
   module may depend on. Cost: mail-shaped knowledge (header names, cue
   phrases) in the AI crate.
2. *Rules in the conversations module, harness depending on it*: right owner
   for mail knowledge, but conversations does not exist yet, and a dev tool
   depending on a domain module inverts the layering.
3. *Copy the rules into the harness*: the eval would measure code that does
   not ship.

**Harness entry point.** An `xtask` subcommand (chosen; the brief asks for it,
and xtask already compiles `campbooks-ai`) vs a `[[bin]]` in `campbooks-ai`
(would add CLI dependencies to the library's dependency set) vs a new
`campbooks-ai-eval` crate (cleanest isolation, one more crate to maintain).

**Metrics.** Coverage at a target precision (chosen as the headline: it is the
auto-act rate the product cares about) vs plain accuracy (hides that a model
can be right on average and wrong where it is confident) vs AUROC (threshold
free, but does not say how much mail can be acted on).

**Runtime selection per question.**
1. *Static policy per question from the eval, recorded in an ADR, overridable
   per instance* (chosen now): predictable, reviewable, cheap.
2. *Per-workspace selection measured online on the workspace's corrections*:
   adapts to each inbox, but needs a corrections volume we won't have at
   launch and machinery to re-evaluate continuously. The same harness and
   metrics make it a natural next step.
3. *Always the model, with rule outputs as extra state*: one code path, but
   pays model latency on every message and inherits the model's calibration
   problems.

**Copying the Rails app.** The `rules` strategy reuses one idea from the
Rails app on `main`: classifying senders as people or services from bulk
headers and sender patterns, and the lesson that some providers do not expose
those headers. No code or structure is copied; the idea wins on the merits
because header signals are free, instant and highly precise when present,
and the eval set includes header-less items to keep the rules honest.

## Risks

- **Synthetic data is easier than real mail.** Mitigated by the hard-case
  quota and header-less items; the private corrections export is the real
  test. The report says which set it ran on.
- **Rule overfitting.** Rules are tuned on `dev` only; `test` is reported.
- **Small n.** ~40 test items per language and question: Wilson intervals are
  shown and decisions are made on the all-language pool first.
- **Model runs could not be made here** (no endpoint, no local Docker): those
  strategies are implemented and tested against wiremock (and the harness
  path against a throwaway local fake), but their numbers wait for a
  configured endpoint (`openspec/decisions-needed.md`).
- **The rules overfit `dev`.** On `dev` most rules are near 100 %; on `test`
  several operating points do not hold (stream 87 % against a 95 % target,
  suspicious recall 56 %). That gap is the honest measure of hand-written
  rules on unseen mail, and the reason the per-question choice needs the
  model numbers.
