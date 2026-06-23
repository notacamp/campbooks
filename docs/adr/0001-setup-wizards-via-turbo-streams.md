---
status: accepted
---

# Setup wizards advance and complete via Turbo Streams

## Context

The setup modals (workspace, AI provider, document types, tags) open a `<dialog>` whose body is a `<turbo-frame id="setup_modal_frame">`. Two incompatible step engines had grown up inside it: the workspace wizard advanced by a full server round-trip per step (re-rendering the frame), while the other three stepped client-side via the `multi-step-form` Stimulus controller and submitted once at the end. Worse, every wizard's final step responded with `redirect_back` (a 302). Turbo, navigating a frame, looked for `setup_modal_frame` in the redirect target, found none, and rendered its literal fallback "Content missing" into the modal, on success. The save had already happened, so users saw a broken screen at the exact end of the flow (a peak-end failure), and any validation message sent the same way was invisible.

## Decision

Setup steps are driven by Turbo Streams inside the dialog, one engine for all four wizards. Each step persists as it advances. The final step never redirects: it responds with a Turbo Stream that (1) refreshes `#setup_banner`, (2) appends a success toast, and (3) closes the `<dialog>` via a small `dialog-close` Stimulus controller rendered into the frame. Validation errors re-render the form *inside* the frame with an inline alert, so they are always visible.

## Considered Options

- **Promote workspace to a full page** (reuse `OnboardingProgress`), keep the rest modal. Rejected: splits setup across two surfaces and two mental models for no user gain.
- **All client-side stepping, single submit.** Rejected: loses per-step persistence, so closing the dialog mid-flow discards everything.

## Consequences

Shipped in two increments. (1) The terminal-state repair: every final step now responds with the Turbo-Stream completion (refresh banner + toast + close) and errors render in-frame. (2) The shortening redesign: workspace collapsed 5 steps to 3 (name+type, context, location+tax), AI collapsed 3 to 2 (auto-named, key), and Document Types and Tags became single-screen starter packs (`SetupPresets`) that create many at once with on-brand colors and pre-written prompts.

The Turbo-Stream *completion* (close + toast + banner) and the in-frame error pattern are unified across all four wizards. The *stepping* mechanism is now matched to each form's shape rather than forced into one engine: pick-lists (types/tags) are a single screen with no stepper, the AI wizard steps client-side via `multi-step-form`, and the workspace wizard steps server-side (persisting each step). This removed the original "two incompatible steppers for the same pattern" defect without a heavier rewrite.
