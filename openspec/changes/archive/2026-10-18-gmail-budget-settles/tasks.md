## 1. Budget

- [x] 1.1 A push reserves its plan; its calls are counted as made (`Push::spent`)
- [x] 1.2 `settle`: give back what was not spent (capped at the budget), owe what was spent beyond (below zero)
- [x] 1.3 The Gmail run settles after its push
- [x] 1.4 Document the rate: a run larger than the budget waits for a full one and owes the rest

## 2. Tests

- [x] 2.1 A push's counted units, as planned and with a refused batch (fake Google)
- [x] 2.2 Settling gives back, owes and waits, and never fills past the budget
- [x] 2.3 A take larger than the budget is charged in full when settled
- [x] 2.4 What a push plans: a batch, singles, trash
