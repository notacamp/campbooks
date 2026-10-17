# Proposal

## Why

The action catalogue (#546) asks its likelihood questions in the same
`decide` call as the triage questions. The independent review found that,
with the catalogue switched on, one catalogue answer the model leaves out or
gets wrong (missing, not a number, out of range) fails the whole call: the
message then gets no triage at all. Turning the catalogue on could make
sorting worse, which it must never do. Two smaller findings: the eval asks
every item about the attachment even when production would not, and the
labelling export can hand the owner a later reply instead of the message
that raised the need.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/166

## What Changes

- Modules touched: `campbooks-ai` (decide parsers, gateway, model strategy),
  `conversations` (labelling export) and `xtask` (eval).
- A catalogue question (`action_*`) whose answer is missing or does not fit
  its type is left unanswered: no rating for that action, nothing else
  changes. Done in the Laya and OpenAI-compatible parsers, scoped to the
  catalogue's ids. The triage questions stay strict exactly as today: a
  missing or unfitting triage answer still fails the call.
- `cargo xtask eval actions` asks each item exactly what the model pass
  asks: Open attachment only for an item with an attachment.
- `labelling export --set actions` lists, per conversation, the newest
  message that raised a need, the message the pass rates, instead of the
  newest inbound one.

## Decisions recorded

- `max_tokens` 4096 stays global; a per-provider override only if a
  self-hoster hits the limit.
- No backfill of older mail: ratings start with mail decided once the
  catalogue is on.
- Comparing triage with the catalogue on and off over the owner's labelled
  eval is the owner's step before switching it on.

## Non-goals

- No change with the catalogue off.
- No OpenAPI change. Note for the Home PR: the schema types `actions` as
  `array|null`, but the server only ever omits the key; the client treats a
  missing `actions` as the fallback.
