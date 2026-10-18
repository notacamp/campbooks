## MODIFIED Requirements

### Requirement: Projecting conversation needs
The handler `attention.project_conversation_need` SHALL consume
`conversations.need_changed` v1 and keep one item per conversation: it SHALL
open or update the item when the event carries a need, clear it with the
event's cause when it does not, and apply an event only when its revision is
newer than the item's, also when two deliveries for a conversation with no
item yet run at once (parallel workers, a replay): the write itself SHALL
only ever move the item's revision forward, so the older one, landing
second, changes nothing and announces nothing, while a newer one landing
second on the row the other inserted is announced (a clear included), with
that row's id. A clear that arrives before any open SHALL leave a
tombstone (a cleared item with no reason, at the clear's revision) that no
list shows, so a late open of an older revision cannot reopen it. When the
event's newest inbound message (`latest_message`) differs from the item's,
every snooze of the item SHALL end. The item table SHALL be rebuildable by
replaying that handler from the event log; snoozes SHALL survive a rebuild.

#### Scenario: Deliveries out of order
- **WHEN** revision 3 of a conversation's need is delivered before revision 2
- **THEN** the item reflects revision 3 and revision 2 changes nothing

#### Scenario: A clear before its open
- **WHEN** the clear at revision 2 is delivered before the open at revision 1
- **THEN** the item is cleared at revision 2, the open changes nothing, and nothing reaches Home or Done today

#### Scenario: Any order
- **WHEN** a conversation's opens and clears are delivered in any order
- **THEN** the item ends in the state of the newest revision

#### Scenario: Replay
- **WHEN** the items table is emptied and `attention.project_conversation_need` is replayed from the start of the log
- **THEN** the same items exist in the same states, and a member's snooze still hides its item

#### Scenario: Two deliveries racing for a new item
- **WHEN** revisions 1 and 2 of a conversation with no item are handled at once and revision 2 is written first
- **THEN** the item stays at revision 2, and revision 1 (an open or a clear) changes nothing

#### Scenario: A newer clear landing on a racing open
- **WHEN** the open at revision 1 and the clear at revision 2 of a conversation with no item are handled at once and the open is written first
- **THEN** the item is cleared at revision 2 and the clear is announced with the item's id
