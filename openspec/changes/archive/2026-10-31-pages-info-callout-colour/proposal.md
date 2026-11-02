## Why

A page's info callout was drawn with the workspace accent. On the default
(ink) workspace that is near-black at 8%, so it looked almost the same as the
neutral callout. The owner asked for its own colour (2026-10-31).

Tracking: https://github.com/notacamp/campbooks-cloud/issues/408 (epic #363).

## What Changes

- New semantic colour tokens in `@notacamp/ui`: `--info` and `--info-soft`, a
  blue (hue 245) defined for light and dark with `light-dark()`, in the same
  pattern as `--danger` and `--success`. The hue is apart from every accent
  preset (ink 70, personal 38, forest 158) and from danger (25) and success
  (155), so the callout keeps its meaning whatever the workspace accent.
- The info callout uses them: a soft blue fill and a blue bar. Neutral stays
  grey and warning stays red.
- The demo page with every block gains a neutral callout, so all three tones
  sit together.

Client only; no server, API or flag change.
