## Design
`Visor` keeps one component. For `dizzy`, `starry` and `hearts` it renders two inline `<svg aria-hidden>` glyphs (24 x 24 viewBox, `currentColor`) instead of the two pill `<i>` eyes; the other states are untouched. All geometry is a multiple of the visor size `--s`, as today. Colour comes from `--visor-eye`; when `tinted`, `data-tinted="true"` switches starry and hearts glyphs to the tint tokens. Motion is CSS keyframes on `rotate`, `scale` and `translate`, with durations from tokens. Under `prefers-reduced-motion` the visor, the pills and the glyphs have no animation.

Data ownership, events and scopes: none (client-only).

## Alternatives considered
1. **Inline SVG glyph eyes (chosen).** Exact shapes (a spiral, a four-point star, a heart) with no extra assets, `currentColor` makes tint a one-line override, scaling is free, and they stay out of the accessibility tree with `aria-hidden`. Cost: three small path constants in `Visor.tsx`.
2. **CSS shapes** (`clip-path`, borders, pseudo-elements on the existing `<i>`). No markup change, but a spiral cannot be drawn faithfully, hearts and stars need brittle clip paths that blur at 22px, and glow (`drop-shadow`) follows the box rather than the shape.
3. **Icon font or sprite sheet.** Reusable, but adds a build asset and a request for three glyphs, and fights the "no raw values, everything a token" rule.

Tint decision:
1. **White by default, tint opt-in (chosen).** The face stays one calm colour system and most callers (a 22px button) look the same; a warm moment can opt into a gold star or rose heart. Tokens are theme-aware (`light-dark()`), so the tint is legible on the glass in both themes.
2. **Always tinted.** More expressive, but every use then needs a colour that works on every accent, and the dark glass already carries white eyes well. Rejected.

Thinking motion: option A (eyes look up and drift, with a blink) reads as pondering but is hard to see at 22px; option B (hop in turn) reads as busy at every size. B is chosen, as approved.

Does this copy a pattern from the old Rails app on `main`? No.
