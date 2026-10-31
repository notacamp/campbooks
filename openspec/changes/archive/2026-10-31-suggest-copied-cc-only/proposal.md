# Suggestions from received mail come from Cc only

Modules: `conversations` (the `conversations.list_correspondents` read).
Tracking: https://github.com/notacamp/campbooks-cloud/issues/404. Follow-up
of campbooks-cloud#395. No migration, no tracking patch, no client change.

## Why

#395 asked for "any person in CC of any email" to be suggested in To, Cc and
Bcc. It also took the other To recipients of received mail. The owner chose
Cc only.

## What changes

- `list_correspondents` reads the Cc of received mail, no longer its other
  To recipients. Senders of received mail and the To and Cc of sent mail are
  unchanged, and so are the ranking and the safety rules for people only
  copied.

## Impact

- `conversations`: one clause of the `list_correspondents` query, its doc
  comments and its HTTP tests. The changelog entry of #395 (not yet released)
  says Cc.
