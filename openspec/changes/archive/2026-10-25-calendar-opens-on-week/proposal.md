# Proposal

## Why

The Calendar opens on Agenda everywhere. On a wide screen the owner wants to see
the week as a grid the moment the place opens; on a phone Week is seven narrow
columns, so Agenda stays the better first view there.

## What Changes

- With no `view` in the address, the Calendar opens on **Week** on a wide screen
  and on **Agenda** below the phone breakpoint (48rem, the one the shell uses).
- A `view` in the address always wins, at either width. The address omits the
  view only when it equals that width's default, so Agenda on a desktop is
  `?view=agenda` and Week on a phone is `?view=week`; a month cell's day number
  still opens that day in Agenda, with the view in the address.
- The view keys stay 1, 2 and 3 for Agenda, Week and Month. Only the default changes.

Module touched: `schedule` (client only). No API, contract or string change.
