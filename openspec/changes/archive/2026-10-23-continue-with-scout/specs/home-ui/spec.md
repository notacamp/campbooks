## ADDED Requirements

### Requirement: Continue with Scout on a need
Each need on Home SHALL offer "Continue with Scout" (the visor before it,
`.` with its hint), after its own actions and in the same button style,
opening the Scout panel for the need's conversation. It SHALL not take one
of the keys 1 to 3. On a server where Scout can't chat it SHALL still be
offered, and the panel SHALL say why.

#### Scenario: Asking about a bill
- **WHEN** Sam selects Marta's Pay need and presses `.`
- **THEN** the Scout panel opens about Marta's conversation, and the need stays selected
