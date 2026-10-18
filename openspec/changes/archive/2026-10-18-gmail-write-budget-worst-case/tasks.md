## 1. Budget

- [x] 1.1 `Writes::most`: each message's own calls, plus a `batchModify` per 11 messages with labels to change
- [x] 1.2 `units` reserves that, plus the profile reads and history pages

## 2. Tests

- [x] 2.1 A refused batch, and every split of a 33-message group, stay within the reservation
- [x] 2.2 Singles joined into a batch, and a trash dropped into a batch, stay within it
- [x] 2.3 A few singles reserve exactly what they plan
