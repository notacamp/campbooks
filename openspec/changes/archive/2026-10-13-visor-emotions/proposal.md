## Why
The Visor (the assistant's face) has seven states, all calm. Errors, big wins and warm moments have no face of their own, and the `thinking` state (flat eyes pulsing) reads as "loading" and is hard to see at 22px. The owner approved a prototype on 2026-10-13 (part of notacamp/campbooks-cloud#89).

## What Changes
- New states `dizzy` (spiral eyes spinning, slight whole-visor wobble: something failed on Campbooks' side), `starry` (four-point stars twinkling: a big win) and `hearts` (heart eyes with a gentle beat: a warm moment).
- `thinking` becomes "working": round eyes hopping in turn, like a typing indicator.
- Optional `tinted` prop: stars and hearts take `--visor-tint-star` / `--visor-tint-heart` (theme-aware tokens). Off by default, eyes stay white (`--visor-eye`).
- Each new state has an `announce` label in en, pt-PT, es-ES and fr.
- New duration tokens for the new motion; `--duration-pulse` is removed (only the old thinking used it).

## Non-goals
- No change to idle, reading, unsure, happy, alert or asleep. The prototype's reworked idle and reading motion is not part of this change.
- No decision on where the emotions are used in screens; callers pick a state.
- No new dependency, no sprite or icon pack.

## Impact
Client design system only (`clients/app/src/lib/ui`). No server, schema, API or event changes.
