## MODIFIED Requirements

### Requirement: Write-back retries and failures
Provider trouble while writing (429, 5xx, a network failure, a refused
token) SHALL stop the push, keep the unwritten changes pending, and use the
run's backoff, throttling and health rules; writes SHALL take Gmail quota
from the shared provider budget: before the push, what it plans (both
profile reads, the history pages it may read, a `batchModify` per chunk and
each single message's calls), then, once it is done, what its calls really
spent, counted call by call: what it did not spend SHALL be given back
(never filling the budget past its two seconds' worth), and what it spent
beyond the plan (a batch Gmail refused, a group the later-writer check
split) SHALL be owed, taking the budget below zero until the rate pays it
back. A run planned to cost more than the budget holds SHALL wait for a full
budget and owe the rest, so no unit goes uncounted whatever rate is
configured. A change Gmail refuses for good, or that
has met trouble eight times, SHALL be recorded `failed` with a reason code
and no content, and the message's Gmail state SHALL be emitted again. A
message Gmail no longer has SHALL be recorded `skipped` (`message_gone`).

#### Scenario: Throttled, then written
- **WHEN** Gmail answers 429 with `Retry-After` to a write
- **THEN** the change stays pending, the stream is throttled until then, and the next run writes it

#### Scenario: A message that refuses changes
- **WHEN** Gmail refuses the change for one message of a conversation
- **THEN** the other messages are changed, that one is recorded `failed` (`refused`), and conversations is told its Gmail state

#### Scenario: A large batch reserves its fallback
- **WHEN** a push plans one `batchModify` of 12 archives and Gmail refuses the batch
- **THEN** it takes 60 units before writing (both profiles, four history pages, the batch) and, once each message is archived on its own, settles the 112 it spent (one profile read before, the batch, twelve modifies, one after): 52 more are owed

#### Scenario: A push spends less than planned
- **WHEN** a push of one batch reads one history page and Gmail takes the batch
- **THEN** the units it did not spend go back to the budget at once
