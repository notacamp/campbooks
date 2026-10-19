# Proposal

Modules: `campbooks-ai` (`decide`: the triage context, the facts),
`conversations` (the upgrade migration). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/273. No new module and no
cross-module dependency.

## Why

The owner reported emails they are only copied on appearing under Needs reply.
The decide model was shown To and Cc merged into one `To:` line and was never
told whether the recipient was addressed or copied, so it could not tell
"kept informed" from "asked".

## What Changes

- **To and Cc are separate lines** in the triage state (`Cc:` only when there
  is a copied recipient).
- **A copied-only hint.** The facts note when one of the inbox's addresses is
  in Cc and none is in To (case-insensitive). The context lists it with the
  other header facts ("the recipient is copied (Cc), not addressed in To") and
  adds one fixed sentence: someone copied on an email is usually being kept
  informed, and it needs a reply from them only when it asks them something
  directly.
- **Context version 3** (state version 4, stamp `checked:ctx3`).
- **Upgrade.** A migration marks the acted `needs_reply` model answers of
  mail copied to the inbox (decided under an older context) not acted and
  removes their `checked:` stamp (`superseded:` prefix), then queues every
  inbox for the needs backfill: the need is withdrawn and the model pass is
  queued again for recent such mail. Other mail is not asked again. Nothing is
  deleted.
- **Labelling.** A ruling in the mail-triage labelling guide: copied-only mail
  is FYI unless it asks the recipient something directly.

## Non-goals

- No rule answers `needs_reply` for copied mail: the facts stay "no" for list,
  automatic and no-reply mail only (ADR 0018). Being asked by name while
  copied is for the model to read.
- No new labelled eval items and no new baseline (the committed baseline is
  not regenerated); the ruling is added to the guide.
