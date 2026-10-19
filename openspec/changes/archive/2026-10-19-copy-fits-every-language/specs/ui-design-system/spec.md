## ADDED Requirements

### Requirement: Translated copy fits
Every string from the UI catalogues SHALL be shown whole in every shipped
locale, at phone (375px) and desktop (1280px) widths: never cut off by an
ellipsis, a clamp or a clipping box, never off screen, and a placeholder
never wider than its field; a screen SHALL NOT scroll sideways. A
translation SHALL be shortened (staying natural) before a component is
changed; a component whose label can grow SHALL wrap it rather than cut it.
User and demo content (names, subjects, addresses) MAY truncate. Every story
SHALL pass this in the "longest" pseudo-locale, where each string is its
longest translation (CI: `pnpm test:fit`).

#### Scenario: A long Spanish label
- **WHEN** the locale is `es-ES` and the navigation shows the inbox
- **THEN** "Bandeja de entrada" is shown whole, with no ellipsis

#### Scenario: Demo content that truncates
- **WHEN** a row shows a sender name longer than its column
- **THEN** the name may end in an ellipsis and the check does not report it
