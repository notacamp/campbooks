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

One item per line:

```json
{"id": "pt-PT-0042", "lang": "pt-PT", "split": "test", "workspace": "atelier",
 "hard": ["deadline_in_prose", "polite_ask"],
 "message": {
   "from": {"name": "Marta Quintela", "address": "marta@quintela-arquitetos.example"},
   "to": [{"name": "Rui", "address": "rui@atelier.example"}],
   "subject": "Plantas do 2.º piso",
   "headers": {"List-Unsubscribe": null},
   "attachments": [{"name": "plantas_v3.pdf", "content_type": "application/pdf"}],
   "body": "Olá Rui, …"},
 "labels": {"stream": "primary", "needs_reply": true, "asks_action": true,
            "has_deadline": true, "urgency": 1, "suspicious": false},
 "rationale": "Pede revisão das plantas «até ao fim da próxima semana»."}
```

- **Workspaces** are four fictional personas (a Lisbon architecture studio, a
  Madrid bakery, a Lyon household, a London freelance consultant), each with
  mail in all four languages, so k-NN can be simulated per workspace.
- **Split** is fixed in the data (`dev` ≈ 50 %, stratified by stream and
  language): thresholds, temperatures and rule tuning use `dev` only; the
  report's headline numbers are on `test`.
- **Mix per language** (≈ 80): primary 40 %, updates 20 %, receipts 20 %,
  newsletters 20 %; suspicious ≈ 10 % (look-alikes of updates and receipts);
  ≥ ⅓ hard cases; ≈ 25 % of items carry no bulk headers at all, because some
  providers don't expose them.
- **Consistency rules** (checked by a test): `needs_reply`, `asks_action` and
  `has_deadline` can be true outside `primary` only where the guide allows it
  (a receipt with a payment due date has a deadline); `urgency = 2` implies
  `has_deadline` or an explicit "today/now"; suspicious items are never
  `needs_reply`. Ids unique; every address on `example.com`, `*.example`,
  `*.test` or `*.invalid`.
- **Labels are produced to spec, then checked blind.** Items are drafted by an
  LLM from a target label vector and a hard-case tag, then a second, blind
  pass labels them from the guide alone. Disagreements are adjudicated against
  the guide and the ruling added to it; items still ambiguous are dropped. The
  report records the agreement per question, which is also a ceiling on what
  any strategy can be expected to reach. I spot-check a sample per language.
- **Versioning:** the manifest has a `version`; changing a label or an item
  bumps it, and reports name the version they ran on.

## Strategy trait

In `campbooks-ai::decide::strategy`, so conversations can use it and the
harness measures exactly what ships:

```rust
pub struct Envelope { from, to, subject, headers, attachments, body, lang_hint }

pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    /// One entry per question, in order; `None` = abstains.
    fn decide<'a>(&'a self, env: &'a Envelope, questions: &'a [Question],
                  memory: &'a dyn Corrections)
        -> BoxFuture<'a, Result<Vec<Option<Answer>>, Error>>;
}

pub trait Corrections: Send + Sync {          // the workspace's labelled examples
    fn examples(&self, question_id: &str) -> &[LabelledExample];
}
```

- `Answer` is the existing type (value, probabilities, confidence, model,
  latency), so a caller's `accept_if(τ)` works the same for every strategy.
- **`rules`** — pure functions of the envelope. Each rule has a fixed
  confidence set on `dev`; abstains when nothing fires. Signals: bulk headers,
  `noreply`-style local parts, attachment MIME and names (`fatura`, `factura`,
  `facture`, `invoice`, `recibo`, `reçu`, `receipt`), amounts with currency,
  date expressions and deadline phrases per language, look-alike sender
  domains and link/display mismatches.
- **`knn`** — embeds the envelope's compact text, takes the k most similar
  corrections of the same workspace (k = 5, distance-weighted), answers the
  majority label with confidence = its weighted share, abstains below a
  minimum number of examples or similarity. Embedders: `GatewayEmbedder` (the
  `embed` role) and `LexicalEmbedder` (hashed character 3–5-grams, no model,
  no dependency) — the latter is a floor, reported under its own name.
  Simulated with each workspace's `dev` items as its corrections, evaluated on
  its `test` items, plus a learning curve at 5, 10, 20 and all corrections.
- **`model`** — `Gateway::decide` with a state built by a shared
  `Envelope::decision_state()` (headers that matter, sender, subject,
  attachment list, first ~1,500 characters of the body). Question wording per
  question lives with the strategy; variants (e.g. `asks_action` as `noul` vs
  a two-option `choice`) are separate named strategies.
- **`cascade`** — `rules>knn>model`: per question, the first stage whose
  answer reaches its stage threshold wins; the rest are not called for that
  question (a model call is skipped entirely when every question is decided).

## Harness

`cargo xtask eval decide` (xtask already builds `campbooks-ai` through the
server; it gains a direct dependency and a tokio runtime for this command):

```
cargo xtask eval decide                          # every runnable strategy, all questions, test split
cargo xtask eval decide --strategy rules --question stream --lang pt-PT
cargo xtask eval decide --strategy 'rules>knn-lexical>laya'
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
  point**: the lowest τ whose precision on `dev` reaches the question's target
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

Output: `<out>/report.md` (tables) and `<out>/report.json` (every number,
dataset version, strategy config, git commit, date). `--check` re-runs the
offline strategies and fails if any operating-point precision or coverage
moves below the committed baseline by more than a tolerance.

## Deciding per question

The eval produces, per question, a ranked list of strategies that reach the
target precision on `test`, ranked by coverage, then cost. The chosen policy
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
1. *Trait and strategies in `campbooks-ai`* (chosen): the only crate both the
   harness and conversations may depend on, so the eval measures what ships.
   Cost: mail-shaped knowledge (header names, keyword lists) in the AI crate.
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
  strategies are implemented and tested against wiremock, but their numbers
  wait for a configured endpoint.
