## MODIFIED Requirements

### Requirement: Backfill
A workspace's index SHALL be built per source in the background, in pages,
resuming where it stopped, starting at the first event or the first query
seen for that workspace, and again whenever the module's index version
changes (as when a source starts reading something it read wrongly before;
an entry whose content stamp changed is then read again). While a backfill runs, queries SHALL answer from what is indexed and
say `indexing: true`, without waiting for the page in progress. The command `campbooks-server search reindex
--workspace <id>` SHALL start one by hand.

#### Scenario: A search during a backfill page
- **WHEN** a backfill page is half-way through, its transaction open, and the member searches
- **THEN** the search answers at once with `indexing: true`

#### Scenario: First search in an existing workspace
- **WHEN** a member searches a workspace whose mail arrived before the module existed
- **THEN** the response says `indexing: true`, and once the backfill ends the same query returns the older mail with `indexing: false`

#### Scenario: A better reading of old mail
- **WHEN** the index version changes because HTML is read better, and a member searches a workspace indexed before
- **THEN** the search answers from the old rows with `indexing: true`, and once the backfill ends the words of mail read from HTML are findable
