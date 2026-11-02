## Why

Notion sat on the Mailboxes page, but Notion is not mail: it is an app
connected to the workspace by one member. The card also said little about
what Notion brings in, how it stays in step, or what disconnecting stops and
keeps.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/382.

Modules touched:

- **Client only** (`clients/app`, `connectors`, the app's Settings frame, one
  link in `pages`): no server, migration, contract, tracked event or prompt
  change.

## What changes

- Settings gets a "Connections" section (`/settings/connections`) between
  Mailboxes and Calendar, on the phone chip row too.
- The Notion card moves there. Mailboxes keeps only mailboxes, with a quiet
  line to Connections; Connections ends with a quiet line to Mailboxes and
  Calendar.
- The connected card shows "Notion · <name>", the health, and "checked Notion
  <ago>" when it is healthy and its stream has a last run (a pull every few
  minutes: never "live"); what it brings in, how it stays in step and what
  Notion shares; one row of actions (Import pages, Open in Pages, Reconnect,
  Disconnect last), two by two on a phone.
- Disconnect asks in a dialog what stops and what stays; behaviour is
  unchanged (the pages stay, nothing is deleted in Notion).
- `/connect/notion` and Pages' "Import from Notion" come back to Connections.
