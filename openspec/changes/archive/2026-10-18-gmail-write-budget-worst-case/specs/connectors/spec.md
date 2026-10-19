## MODIFIED Requirements

### Requirement: Write-back retries and failures
Provider trouble while writing (429, 5xx, a network failure, a refused
token) SHALL stop the push, keep the unwritten changes pending, and use the
run's backoff, throttling and health rules; writes SHALL take Gmail quota
from the shared provider budget before the push starts: the most its writes
can cost, each message's own calls as if it were written alone plus one
`batchModify` for every 11 messages with labels to change, so a batch Gmail
refuses (each of its messages then modified on its own) or a group the
later-writer check splits never spends more than was taken. A change Gmail refuses for good, or that
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
- **WHEN** a push plans one `batchModify` of 33 archives
- **THEN** besides its reads it takes 315 units before writing (165 for the messages, 150 for batches), enough for the batch to be refused and each message archived on its own, or for any split of the group
