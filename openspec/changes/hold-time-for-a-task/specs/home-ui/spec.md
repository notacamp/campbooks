## ADDED Requirements

### Requirement: Hold time from Home
A Do task on Home SHALL offer Hold time (`h`) beside Done and Later, opening
the same Hold time panel as the task's detail in a dialog; a held task SHALL
show its "Held <day> <start>–<end>" chip. A Pay task SHALL not offer it.

#### Scenario: Hold from Home
- **WHEN** Ana presses `h` on "Send the signed lease" on Home and picks a slot
- **THEN** the row shows "Held Wed 10:00–10:30", and a toast offers Undo
