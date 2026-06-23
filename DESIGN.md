---
name: Campbooks
description: Consumer-grade, AI-forward email home for young professionals — minimal white/black canvas, one warm "Ember" signature, Inter-only UI
register: product
colors:
  # ONE warm signature gradient ("Ember") + near-black functional color + white canvas
  # + warm-neutral grays + a functional status set. The full light+dark token system
  # lives in app/assets/tailwind/application.css; values below are the light theme.
  ember-gradient: "linear-gradient(135deg, oklch(62% 0.23 354), oklch(67% 0.21 22), oklch(76% 0.17 60))"
  ember-solid: oklch(64% 0.21 20)
  ink: oklch(17% 0.005 60)        # near-black — THE functional color (primary actions, text, emphasis). Replaces the retired electric-violet.
  ink-2: oklch(42% 0.007 60)
  faint: oklch(62% 0.007 60)
  bg: oklch(100% 0 0)             # white canvas (light); dark = oklch(14% 0.004 60)
  card: oklch(100% 0 0)
  muted: oklch(96.6% 0.002 60)    # hover fills
  subtle: oklch(93.5% 0.003 60)   # avatar / inactive ring fills
  border: oklch(91.5% 0.003 60)
  hair: oklch(94% 0.0025 60)      # list dividers
  scout-bg: oklch(98.2% 0.02 40)  # light Ember "glass" tint behind Scout's contribution block
  status-green: oklch(45% 0.13 152)
  status-amber: oklch(52% 0.12 78)
  status-orange: oklch(53% 0.16 48)
  status-red: oklch(53% 0.18 25)
  status-blue: oklch(48% 0.14 248)
typography:
  family-ui: "Inter, ui-sans-serif, system-ui, sans-serif"   # the ONLY typeface in the product UI
  family-brand: "display faces (Fraunces, Clash Display, grotesques) — brand/marketing layer ONLY, never in the app UI"
  base: "0.875rem"               # 14px
  body: { size: "0.875rem", weight: 400, lineHeight: 1.6 }
  meta: { size: "0.78rem", weight: 500, color: faint }
  card-subject: { size: "1.0625rem", weight: 600, letterSpacing: "-0.015em" }
  greeting: { size: "1.25rem", weight: 600, letterSpacing: "-0.02em" }
  mono: "JetBrains Mono, ui-monospace, monospace"
rounded:
  chip: "0.5rem"
  button: "0.7rem"
  scout-note: "1rem"
  card: "1.375rem"               # 22px — the big feed content cards
  full: "9999px"
components:
  logo: { tile: "{colors.ember-gradient}", mark: "layered-C", glow: true }
  story-ring:
    live: { border: "{colors.ember-gradient}", glow: true }   # has items that still need you
    done: { border: "{colors.border}", badge: "✓" }           # Scout already handled
    action: "tap launches the skim stack (one tap, not select-then-start)"
  content-card:
    surface: "none — flat on the canvas (no border, no shadow, no fill); the box is gone, separation is whitespace"
    spacing: "one dominant card per viewport — generous whitespace between cards; content aligns to the page gutter (one left edge for header, labels, and every card)"
    interactive-rows: "lighter list items (nudge, filing suggestion, doc queue) get a hover fill that bleeds past the content (-mx) so content stays on the gutter — Linear/Notion pattern, never a bordered box"
    anatomy: "sender header → subject → email excerpt → meta row → Scout note → right-aligned action bar"
  meta-row: "small gray chips: category tag · attachments (paperclip + filename) · thread count · priority accent (Ember dot)"
  scout-note:
    background: "light Ember glass gradient (frosted + soft Ember tint over scout-bg)"
    rounded: "{rounded.scout-note}"
    contents: "Scout avatar (Ember) + name + 'AI' tag + 'read it just now' + the message. Reads as an entity's contribution, never a caption."
  action-bar: "right-aligned; priority increases right-to-left; primary = solid {colors.ink} (near-black), secondaries = ghost"
  nudge-card: "lighter passive feed item — 'X sent this a week ago, reply?' + Dismiss/Reply. Feed-only, not a skim item."
  button-primary: { backgroundColor: "{colors.ink}", textColor: "{colors.bg}", rounded: "{rounded.button}" }
  button-ghost: { background: transparent, border: "1px {colors.border}", textColor: "{colors.ink-2}" }
  scout-bar: "glass-docked composer pinned bottom — Ember spark + input + near-black send"
---

# Design System: Campbooks — "Quiet canvas, warm signal"

> **Direction (June 2026): consumer-grade, AI-forward, Instagram-shaped.** This
> supersedes BOTH the original "calm / restrained / system-font" brand AND the
> interim "electric-violet, dynamic-modern (RubyUI)" direction. The electric-violet
> accent (hue 276) is **retired**. The system is now a minimal white/black canvas
> with a single warm **Ember** gradient as the only signature color, Inter as the
> only UI typeface, and a home screen modeled on Instagram (story-rings + a feed of
> big email content-cards). Architecture (semantic tokens + `.dark` + the bridged
> legacy ramps in `app/assets/tailwind/application.css`, the `theme` controller, the
> `Campbooks::Logo` component, Lookbook previews) carries over from the violet
> migration; only the token **values** and the home **screen** change.

## 1. North Star

Campbooks is the email home a young, AI-native professional actually *wants* to open. It looks like a great consumer app, not an email client or a B2B dashboard. The boring work (sorting, reading, summarizing, drafting) is done by Scout before you arrive; your time goes to deciding and conversing. The feel: **alive and warm, but never busy.** See `PRODUCT.md` for the audience and principles.

The whole expressive load rides on **color, motion, and one gradient** — never on a decorative font or a wall of color.

## 2. Color

**Strategy: Restrained.** A clean **white** canvas and **near-black** type carry ~95% of every screen. Warm-tinted neutral grays give structure. The single **Ember** gradient (a magenta→orange sunset) is the one saturated thing, and it means something specific.

- **Ember (the signature).** Appears ONLY on: the logo, Scout (avatar + contribution block), the *live* story-rings, the one primary/delight CTA where it's truly a win, and celebration moments. Strongest on dark, where it glows. Ember = "this is Scout / this wants you / this is a win."
- **Near-black `ink` (the functional color).** Primary buttons, body text, emphasis, active states. This replaced the retired electric-violet. In dark mode it inverts (a white primary button on near-black).
- **Status palette** (green/amber/orange/red/blue): state only, never decoration. A priority accent is a small Ember dot, not a colored block.

**The Meaning Rule.** If a pixel is Ember, it must mean Scout, a live stack, or a win. If color appears without that meaning, remove it. The user's own test: "more white and black, color only for accents."

## 3. Typography

**Inter, everywhere in the UI** — body and headings alike, hierarchy from weight (400/500/600/700) and scale. No display face, no grotesque, nothing decorative inside the app. Expressive display faces (Fraunces, Clash, grotesques) are allowed **only** in the brand/marketing layer (website, posters, logo lockups), never in the product UI.

## 4. Elevation, glass & glow

Depth is real but quiet, and every effect must serve a tight, coherent whole (the user's bar: "glass and stuff is fine as long as the holistic experience makes sense").

- **Glass** (frosted `backdrop-blur` + translucent surface): the sticky topbar, the docked Scout bar, the skim overlay backdrop, and the **Scout note** (a light Ember glass gradient). Purposeful, not decorative.
- **Glow**: a soft Ember drop-shadow on Ember elements (logo, Scout avatar, live rings, primary CTA). It earns its keep on the dark theme.
- **Cards**: flat on the canvas — no border, no shadow, no fill. Separation is **whitespace, not a box** (Instagram-shaped feed); a run of cards shares one left edge at the page gutter. In the feed, spacing is tuned so **one content card dominates the viewport** with the next just peeking — never two full emails competing for attention. (Glass/glow still belong to Scout, the topbar, and the docked bar — depth lives on those, not on feed cards.)

## 5. Components

- **Logo** — Ember gradient tile + the "layered-C" mark, with glow. The brand's one guaranteed splash of Ember.
- **Story rings** — horizontal row at the top of home. Ember gradient border = a stack with items that still need you; gray + ✓ = Scout already handled. **One tap launches the skim stack** (no select-then-start).
- **Content card (the feed unit)** — flat and borderless, content full-bleed to the page gutter, separated from its neighbors by whitespace rather than a box. Anatomy, top to bottom: sender header (avatar · name · time) → subject → real email excerpt → **meta row** (category tag, attachments, thread message count, priority accent) → **Scout note** → **right-aligned action bar**.
- **Scout note** — a light **Ember glass gradient** block: Scout's avatar (Ember) + name + an "AI" tag + "read it just now" + the message. It must read as an *entity contributing*, with a face and a name, not a captioned line of text.
- **Action bar** — right-aligned; priority increases right-to-left; the primary (Scout's suggested action) is solid near-black on the far right, secondaries (Reply, Archive) are ghost to its left. Acting resolves the card inline ("✓ Scout took it from here · Undo").
- **Nudge card** — a lighter, passive feed item Scout surfaces ("Maya sent this 8 days ago, reply?"). Feed-only; it does not appear in skim stacks.
- **Skim overlay** — launched from a ring: a focused, one-at-a-time stack (progress dots, Archive/Snooze/Approve), ending in "Stack cleared ✨."
- **Scout bar** — a glass-docked composer pinned to the bottom, present on every surface (Ember spark + input + near-black send).

## 6. Motion

Exponential ease-outs (`--ease-out-quint` / `--ease-out-expo`), no bounce. Cards fly out on action; rings lift on hover; the celebration is the one indulgent moment. Everything honors `prefers-reduced-motion` (movement removed, never broken).

## 7. Do / Don't

**Do** keep the canvas white/near-black with Ember as the only signature, and make every Ember pixel mean Scout / live / win. **Do** use Inter for all UI type. **Do** let cards breathe, one per viewport in the feed. **Do** make Scout feel like a present entity. **Do** right-align actions, primary on the far right.

**Don't** reintroduce the electric-violet, anywhere. **Don't** put a display/grotesque face in the app UI. **Don't** use color decoratively or pack two full emails into one viewport. **Don't** box feed cards in borders or drop-shadows — they're flat on the canvas, separated by whitespace. **Don't** let Scout degrade back into "an icon next to a line of text." **Don't** add glass/glow that doesn't serve the whole.
