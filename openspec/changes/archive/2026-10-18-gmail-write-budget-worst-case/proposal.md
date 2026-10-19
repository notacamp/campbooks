## Why

A Gmail push takes its quota from the shared provider budget before it
starts, but it budgeted only the writes it planned: 50 units per
`batchModify` chunk. Two cases cost more (campbooks-cloud#228, from the
review of #540):

- **A refused batch:** when Gmail refuses a whole `batchModify`, each message
  is modified on its own at 5 units. A 500-message batch planned at 50 units
  could cost 2,550.
- **Split groups:** the cost came from the merged plan, before the
  later-writer check and "whole or not at all". Overtaken labels can split a
  large group into groups under 11, which are written one by one at 5 units
  each, or into several batches.

Gmail answered 429 and the run backed off, so nothing broke, but the budget
no longer bounded quota use.

## What Changes

- The budget a push takes is the most its writes can cost, not what it
  plans: each message counts its own calls as if written alone (untrash,
  modify, trash, 5 units each), and every 11 messages with labels to change
  count one `batchModify` (50 units) on top. The later-writer check only
  takes labels away, so no message ever needs more calls than that. This
  covers a refused batch, any split, and singles that end up joined into a
  batch.
- Pushes are at most 500 changes, so a push reserves at most about 7,300
  units, under the two seconds' burst of the default shared budget. A push
  of a few messages one by one reserves exactly what it plans.

## Impact

- Specs: connectors (Write-back retries and failures).
- Code: `connectors::gmail::write` (`units`, `Writes::most`), with tests for
  a refused batch, splits of a 33-message group, singles joined into a
  batch, and a trash dropped into a batch.
- No migration. Nothing changes on screen.
