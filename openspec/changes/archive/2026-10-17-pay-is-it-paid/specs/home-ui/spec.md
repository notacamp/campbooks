## ADDED Requirements

### Requirement: Is it paid?
A Pay need that is a question (trust `check`, question `money`) SHALL read
"<Payee> sent an invoice. Is it paid?" (no amount until one is read from the
invoice; never parsed from the mail's words) and offer Already paid (1) and
Still to pay (2), then Snooze (S) and Not needed (X). Already paid SHALL fold
the row into "Marked paid · <payee>" with Undo in the toast. Still to pay
SHALL keep the row in Pay as "Pay <payee>", with "N days overdue" or its due
date when the need has one (from `due_on` only), and offer first a reminder:
two days before the due date at 09:00, tomorrow at 09:00 once that has
passed or the bill is overdue, or next Monday at 09:00 with no due date,
labelled by the time it gives ("Remind me tomorrow at 09:00", "Remind me on
28 October"); then Mark paid (2), Snooze and Not needed. The reminder SHALL snooze the item
until then, with the Snooze toast and Undo. When the Pay group holds two or
more bills asked about, its heading SHALL offer "All N already paid" (Shift+E),
which marks them all paid with one toast and one Undo for all of them (when
some fail, the Undo covers the others and one failure offers Retry for the
rest); the group
SHALL say that Campbooks can't see the bank, so it asks. After Already paid
the screen SHALL leave a hook for offering a paid rule for that sender
(campbooks-cloud#169), which offers nothing yet.

#### Scenario: Already paid
- **WHEN** Sam selects "Acme Hosting sent an invoice. Is it paid?" and presses 1
- **THEN** the server records `already_paid` for the message Sam saw and the row folds into "Marked paid · Acme Hosting"

#### Scenario: Still to pay
- **WHEN** Sam presses 2 on the same bill, due 30 October, on 13 October
- **THEN** the row reads "Pay Acme Hosting" and its first action is a reminder for 28 October at 09:00

#### Scenario: Overdue
- **WHEN** a bill Sam said is still to pay was due yesterday
- **THEN** its row says "1 day overdue" and its reminder is tomorrow at 09:00

#### Scenario: All already paid
- **WHEN** the Pay group asks about two bills and Sam chooses "All 2 already paid", then Z
- **THEN** both are marked paid with one toast "Marked 2 invoices paid", and Z reopens both
