## ADDED Requirements

### Requirement: Moving a draft to another inbox
A draft save SHALL be able to name another `inbox_id`. The target SHALL be
an inbox in the same workspace where the author is a `sender` or `manager`
and that is not read-only. A `reader` SHALL get 403 `inbox_read_only`, and
any other inbox SHALL answer 404. The draft SHALL then be sent from that
inbox's address. A reply SHALL keep its `In-Reply-To` and `References`, and
drop the first mailbox's thread key.

#### Scenario: Reply from the other mailbox
- **WHEN** Sam moves a reply draft from personal@ to office@, where Sam is a sender, and sends it
- **THEN** it leaves from office@ with the original `In-Reply-To` and `References`

#### Scenario: An inbox Sam only reads
- **WHEN** Sam moves a draft to an inbox where Sam is a reader
- **THEN** the response is 403 `inbox_read_only` and the draft is unchanged

#### Scenario: Too much for the new mailbox
- **WHEN** a draft with 20 MB of attachments is moved to an inbox whose mailbox carries about 2.1 MB
- **THEN** the response is 413 `attachments_too_large_for_mailbox` naming the limit, and the draft stays where it was
