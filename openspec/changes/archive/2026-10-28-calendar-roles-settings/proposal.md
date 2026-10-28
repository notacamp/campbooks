# Proposal

## Why

The change `calendar-roles` gave every calendar a role on the server: Mine,
Following or Off (campbooks-cloud#399). The member still sees only an on/off
switch per calendar, so they cannot tell their calendars from the ones they
follow, or choose which calendars hold their tasks.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/399

## What Changes

- Settings → Calendar groups calendars under **Mine** ("Make you busy, hold
  your tasks, and send their invitations to Home"), **Following** ("Shown
  only") and **Off** ("Hidden"). A segmented control (Mine / Following /
  Off) on each provider calendar replaces the switch. A calendar the provider
  says the member does not own says "Shared with you". A role change moves
  the calendar at once and offers Undo.
- The calendar pickers for Hold time, "Add to my calendar" and "Also add to"
  offer only writable Mine calendars. An "Also add to" target that is no longer
  Mine shows as unset.
- Strings in en, pt-PT, es-ES and fr, fitting 375px.

Module touched: `schedule` (client) only. No API, contract or migration
change.

## Non-goals

- A different look for Following calendars on the Calendar views.
