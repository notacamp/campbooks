## ADDED Requirements

### Requirement: Needs cleared by a rule
The attention projection SHALL accept a need cleared with cause `rule` and
treat it as cleared for every member who can see the container, without
listing it among what left the member's list today.

#### Scenario: A rule says never
- **WHEN** `need_changed` clears a Pay need with cause `rule`
- **THEN** the item leaves every member's list and is not shown in Done today
