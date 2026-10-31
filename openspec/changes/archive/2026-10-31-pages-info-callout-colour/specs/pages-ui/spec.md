## ADDED Requirements

### Requirement: Callout tones are told apart
A callout SHALL show its tone by colour: neutral in grey, info in blue,
warning in red, each with a soft fill and a bar on its leading edge, in light
and dark themes. The info colour SHALL be its own design token, not the
workspace accent, so it reads the same whatever accent the workspace uses.

#### Scenario: Info and neutral callouts side by side
- **WHEN** a page has an info callout and a neutral callout
- **THEN** the info callout is blue and the neutral one grey, in light and in dark

#### Scenario: A workspace with a coloured accent
- **WHEN** the workspace accent is forest
- **THEN** the info callout is still blue
