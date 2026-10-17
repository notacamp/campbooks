## ADDED Requirements

### Requirement: Clamp
`lib/ui` SHALL provide `Clamp`: long content held to `--clamp-height` with a
fade (`--clamp-fade-height`) and a link button that shows it all in place and
folds it back, saying which with `aria-expanded`. The button SHALL show only
when the content is taller than the clamp.

#### Scenario: A long email on Home
- **WHEN** an opened item's email is taller than the clamp
- **THEN** it ends in a fade with "Show the whole email", and pressing it shows all of it with "Show less"
