## Why

#576 (campbooks-cloud#228) made a Gmail push reserve its worst case, up to
about 7,300 units, from the one shared Google budget, which is take-only.
Its review found three problems (campbooks-cloud#252):

- **Held, not spent:** a typical 500-message archive spends about 60 units
  yet held about 4,760 for that second, and nothing gave them back.
- **Starvation:** a blocked big push is rescheduled after
  `(cost - available) / rate`, and smaller reads can drain the budget
  meanwhile, again and again.
- **Clamp:** `take` caps a cost at the budget's two seconds' worth, so
  below about 3,900 units a second the worst case was silently undercounted.

## What Changes

- **Plan, then settle.** A push takes what it plans before writing: both
  profile reads, the history pages it may read, a `batchModify` per chunk,
  and each single's calls. Its calls are counted one by one as they are
  made (refused ones too), and once it is done the run settles the
  reservation with what was really spent.
  - Unspent units go back at once, never past the budget's size.
  - Units spent beyond the plan (a refused batch, a split group) are owed:
    the budget goes below zero and later takes wait until the rate has paid
    it back.
- **Why this over a fair queue:** a big push now needs only its plan (a
  batch is 50 units), so it is no longer outwaited by smaller runs, and
  this needs no queue state or migration. Every unit is still counted
  exactly, just after the fact. A burst can exceed the budget by at most
  one push's extra calls (bounded by the 500-change push limit), which
  Gmail's own 429 and the run's backoff already handle.
- **Clamp:** a run planned to cost more than the budget holds waits for a
  full budget, takes it, and owes the rest when settled. Whatever the
  configured rate, no unit goes uncounted; a low rate only slows big
  pushes. This is documented on `CAMPBOOKS_CONNECTORS__GOOGLE__UNITS_PER_SECOND`.

## Impact

- Specs: connectors (Write-back retries and failures).
- Code: `connectors::gmail::write` (`units` is the plan; a metered API
  counts what the push spent into `Push::spent`), `domain::schedule`
  (`settle`, `settle_budget`), `sync_stream` (settles after the push),
  config docs, `deploy/.env.example`.
- No migration. Nothing changes on screen.
