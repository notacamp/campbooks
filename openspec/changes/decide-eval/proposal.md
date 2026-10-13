# Proposal

## Why

The first real Laya run (campbooks-cloud#58, ADR 0009's dated note) was fast
on CPU but confidently wrong zero-shot: "newsletter" at 0.9997 for a
Portuguese invoice, and P(true) ≈ 0.003 that an email asking for a payment
date "asks the recipient to do something". ADR 0009 now says a question's
answers are suggestions until an eval shows they can be acted on. Nothing
measures that today, so no triage question can ever leave suggestion mode,
and we cannot tell whether Laya, an instruct model, cheap header rules or the
user's own corrections are the right tool for each question.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/59
(sub-task of #20, the AI gateway). Module touched: none; `campbooks-ai` and
`xtask` only.

## What Changes

- **Eval set** `mail-triage` v1 under `crates/ai/evals/decide/mail-triage/`:
  about 320 fictional, labelled messages (80 each in `en`, `pt-PT`, `es-ES`,
  `fr`), JSONL, with a manifest (the questions and their label sets), a
  labelling guide, a fixed `dev`/`test` split and a fictional `workspace` per
  item. Every label answers one of the six triage questions of the
  conversations module: stream (`primary`, `updates`, `receipts`,
  `newsletters`), needs a reply, asks the recipient to do something, has a
  deadline, urgency (0–2) and suspicious. At least a third of each language is
  hard cases (polite asks, deadlines in prose, receipts vs invoices,
  newsletters from people, phishing look-alikes, quoted threads, mixed
  languages). Only reserved domains (`example.com`, `.example`, `.test`) and
  invented names.
- **Strategy trait** in `campbooks-ai::decide::strategy`: one interface that
  answers triage questions about a message envelope, may abstain per question,
  and returns the same `Answer` type as `Gateway::decide`. Implementations:
  - `rules`: deterministic signals (List-Unsubscribe / List-Id / Precedence /
    Auto-Submitted, attachment types, no-reply senders, per-language keyword
    and date patterns, look-alike domains); abstains when no signal fires;
  - `knn`: k nearest neighbours over the workspace's own corrections, on top
    of an `Embedder`: the `embed` role (any OpenAI-compatible endpoint), or a
    dependency-free lexical embedder (hashed character n-grams) as a floor;
  - `model`: `Gateway::decide` with either protocol (Laya or an instruct
    model), with the question wording and the state builder the module will
    use;
  - `cascade`: try strategies in order, each accepted above its own
    threshold, per question.
- **Harness** `cargo xtask eval decide [--strategy …] [--question …]
  [--lang …] [--split test] [--out <dir>]`: runs strategies over the set and
  writes a markdown and a JSON report per question × language × strategy:
  accuracy, coverage and precision at thresholds with the operating point
  (lowest threshold that reaches the question's target precision on `dev`,
  then measured on `test`), Wilson intervals, recall of the positive class,
  calibration error and Brier score before and after a temperature fitted on
  `dev`, and latency. Strategies that need an endpoint run only when their
  `CAMPBOOKS_EVAL_*` variables are set, and are reported as **not run**
  otherwise.
- **Dataset check** in `cargo test`: schema, label consistency rules (e.g.
  `needs_reply ⇒ stream = primary`), unique ids, reserved domains only.
- **Baseline report** committed for the strategies that run without an
  endpoint (rules, lexical k-NN, cascades of them), and an offline regression
  check (`cargo xtask eval decide --offline --check`) in CI.

## Capabilities

### New Capabilities

- `ai-decide-eval`: how a `decide` question's quality is measured (eval set,
  metrics, operating point) and how a question is answered by a chosen
  strategy instead of always by the model.

### Modified Capabilities

None (the `ai-decide` delta from `decide-role-laya` is unchanged).

## Impact

- Crates: `campbooks-ai` (new `decide::strategy` module, eval set files) and
  `xtask` (new `eval` subcommand). No module, endpoint, migration, OpenAPI or
  client change. No new crate dependency planned (`xtask` already has the
  workspace's `tokio`, `clap`, `serde_json`; it gains `campbooks-ai`, already
  in its build graph through the server).
- CI: one new offline step (rules + lexical k-NN, milliseconds).
- Self-hosters: nothing changes at runtime.

## Non-goals

- Choosing the production strategy per question in this change when a model
  could not be run: the ADR amending 0009 lands with real model numbers (see
  design, "Deciding per question").
- Wiring triage into conversations (campbooks-cloud#37); it consumes the
  trait and the policy.
- Storing corrections, refitting temperatures in production, per-workspace
  thresholds.
- Fine-tuning or distilling Laya; the harness evaluates whatever checkpoint
  the configured endpoint serves, so a fine-tuned one plugs in later.
- Running Laya or any model locally in Docker.
- Real users' mail in the repo. Opt-in labelled corrections are evaluated by
  the same harness from a private export, never committed.
