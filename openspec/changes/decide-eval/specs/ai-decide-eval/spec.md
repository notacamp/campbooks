## Purpose

How Campbooks measures whether a `decide` question can be acted on, and how a
question is answered by the strategy that the measurement chose: a versioned,
fictional eval set, one strategy interface, and a harness that reports the
auto-act operating point per question and language.

## ADDED Requirements

### Requirement: Versioned fictional eval set
The repository SHALL hold a labelled mail-triage eval set with a manifest
naming its version, its questions and each question's label set and target
precision, and a labelling guide. Every item SHALL carry an id, a locale
(`en`, `pt-PT`, `es-ES` or `fr`), a split (`dev` or `test`), a fictional
workspace, the message envelope and one label per question. Items SHALL use
only invented names and reserved domains, and SHALL satisfy the guide's
consistency rules; a test SHALL fail otherwise.

#### Scenario: A real-looking domain slips in
- **WHEN** an item's sender address is on a domain that is not reserved for examples
- **THEN** the dataset test fails, naming the item

#### Scenario: Inconsistent labels
- **WHEN** an item is labelled suspicious and needs a reply
- **THEN** the dataset test fails, naming the item and the rule

### Requirement: One strategy interface
`campbooks-ai` SHALL offer one strategy interface that answers a list of
typed questions about a message envelope with the same answer type as
`decide`, and MAY abstain per question. Deterministic rules, k-NN over a
workspace's corrections, a `decide` model, and an ordered cascade of these
SHALL implement it.

#### Scenario: Rules abstain
- **WHEN** no rule fires for a question
- **THEN** the rules strategy returns no answer for it and makes no request

#### Scenario: A cascade decides without the model
- **WHEN** earlier stages answer every question above their thresholds
- **THEN** the cascade makes no model call

### Requirement: Eval harness and report
`cargo xtask eval decide` SHALL run the selected strategies over the selected
questions, locales and split, and write a markdown and a JSON report with, per
question, locale and strategy: accuracy and answered rate, coverage and
precision at thresholds, the operating point (the lowest threshold reaching
the target precision on `dev`) measured on `test` with a 95 % interval,
calibration error and Brier score before and after a temperature fitted on
`dev`, and latency. The report SHALL name the dataset version.

#### Scenario: Strategy without an endpoint
- **WHEN** a strategy needs an endpoint whose `CAMPBOOKS_EVAL_*` variable is unset
- **THEN** no request is attempted and the report lists the strategy as not run, with the missing variable

#### Scenario: Offline regression
- **WHEN** `cargo xtask eval decide --offline --check` runs and an offline strategy's operating-point precision or coverage falls below the committed baseline by more than the tolerance
- **THEN** the command fails, naming the question, locale and strategy
