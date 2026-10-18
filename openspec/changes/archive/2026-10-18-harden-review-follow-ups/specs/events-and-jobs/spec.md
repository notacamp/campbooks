## ADDED Requirements

### Requirement: The queue's key indexes are valid after migrating
After the migrations have run, by the server or by `cargo sqlx migrate run`,
the job queue's per-workspace unique-key and ordering indexes SHALL exist and
be valid. A migration SHALL build again, in its transaction, each one that is
missing or invalid (what a failed concurrent build leaves), and SHALL leave a
valid one alone.

#### Scenario: An invalid unique-key index
- **WHEN** the unique-key index was left invalid, or the ordering index missing, by a migration run that does not drop invalid indexes first, and the migration runs
- **THEN** both indexes exist and are valid, enqueues with a unique key work, and a valid index is not rebuilt
