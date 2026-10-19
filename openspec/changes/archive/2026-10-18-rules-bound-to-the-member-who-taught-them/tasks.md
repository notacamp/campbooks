# Tasks

## 1. The binding

- [x] 1.1 `domain::rules::binding`: the content hash (`campbooks/conversations.rule-content/v1`) and the rule's MAC (`campbooks/conversations.rule/v1`) over the workspace, rule, generation, inbox, author, state, precedence and content hash; `Stored::bound`, `needs_confirmation`, `acts`; `sign` (the next generation; 500 without a key)
- [x] 1.2 `Stored` carries the workspace, generation and MAC (read next to the row, no new compile-time query); a row that does not decode is left out with a WARN

## 2. Signing in members' requests

- [x] 2.1 `create_rule` signs an active rule; `confirm_rule` signs a proposal at the yes, and confirms again a kept rule that is not bound
- [x] 2.2 `update_rule` locks and verifies first, signs again only a bound rule, and answers 409 `rule_needs_confirmation` for anything but a pause on an unbound one
- [x] 2.3 `remove_rule`, `restore_rule` and `retire` verify under the lock and sign again in the new state

## 3. Verifying before any effect

- [x] 3.1 `rules::acting` leaves out an unbound active rule (WARN, ids only): arrival, runs and "Always show images"
- [x] 3.2 `apply_rule` stops an apply run of an unbound rule before its first change (WARN, cancelled, no retry)
- [x] 3.3 `RuleView.needs_confirmation`; OpenAPI and the client's generated types

## 4. The deploy

- [x] 4.1 Migration `20261022000000`: `rules.mac`, `rules.binding_generation`, the snapshot `rule_resigns` (read-only for the runtime role), `take_rule_resign` (definer, owned by `campbooks_definer`); rollback SQL in `design.md`
- [x] 4.2 `conversations.dispatch_rule_resigns` (maintenance scope, `architecture.toml`) and `conversations.resign_rules` (signs only what the function confirms unchanged)

## 5. Tests

- [x] 5.1 Unit: every bound field, a copied MAC, another key, a proposal, the content hash
- [x] 5.2 A rule planted with SQL (no, made-up or malformed MAC), and its planted run, change nothing and request nothing; it is listed as needing a yes
- [x] 5.3 A signed rule changed with SQL (actions, conditions, author, inbox, precedence, generation, paused or removed state brought back) stops acting
- [x] 5.4 A MAC copied onto another rule, inbox or workspace does not verify
- [x] 5.5 Teach, change, pause, resume, remove and restore, then arrival, still act and request their changes; a rule changed with SQL is only paused until confirmed again, then acts
- [x] 5.6 A rule that does not decode, or is not bound, never fails ingest
- [x] 5.7 The deploy snapshot signs the untouched rule only; an edited or planted one waits; the runtime role cannot write the snapshot; a planted job takes only what is there; without a key nothing is taken
- [x] 5.8 Server: the definer role's privileges
