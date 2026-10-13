## ADDED Requirements

### Requirement: Visor emotions
The Visor SHALL offer the states `dizzy`, `starry` and `hearts` in addition to idle, reading, thinking, unsure, happy, alert and asleep. In these three states each eye SHALL be an inline SVG hidden from assistive technology. `dizzy` SHALL spin its spiral eyes and wobble the visor slightly, `starry` SHALL twinkle four-point stars, and `hearts` SHALL beat gently. With `announce`, each state SHALL expose a label in English, Portuguese (pt-PT), Spanish (es-ES) and French.

#### Scenario: Glyph eyes
- **WHEN** a Visor renders with state `starry`
- **THEN** it contains two `svg` eyes with `aria-hidden="true"` and no pill eyes

#### Scenario: Announced
- **WHEN** a Visor renders with state `dizzy` and `announce`
- **THEN** it has role `img` and a localized accessible name

### Requirement: Working motion for thinking
The `thinking` state SHALL show round eyes that hop in turn, so it reads as busy at the smallest size (22px).

#### Scenario: Eyes take turns
- **WHEN** the Visor is in the `thinking` state
- **THEN** the second eye's hop is delayed relative to the first

### Requirement: Optional tint
The star and heart eyes SHALL be the token-based white `--visor-eye` unless the `tinted` prop is set, in which case they SHALL use `--visor-tint-star` and `--visor-tint-heart`, both defined for light and dark themes.

#### Scenario: Off by default
- **WHEN** a Visor renders without `tinted`
- **THEN** `data-tinted` is `false` and the eyes use `--visor-eye`

### Requirement: Reduced motion
Under `prefers-reduced-motion: reduce`, every Visor state SHALL render without animation.

#### Scenario: Motion preference
- **WHEN** the user prefers reduced motion
- **THEN** no Visor element animates
