## Why

The client works out whether mail and Scout in pages is on by asking
`pages.get_sharing` about a page: 200 means on, 404 means off. That costs an
extra request, and a workspace with no pages yet is not offered "Save to
page" until it has one, though "New page" is a main reason to use it.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/403.

Modules touched:

- **pages** (server): `pages.list` answers `mail_scout` next to `tables`, from
  the existing `pages.mail_scout.enabled` config. Additive field; no
  migration, tracked event or prompt change.
- **Client** (`clients/app`): `useMailScoutAvailable` reads it from the list the
  tree already holds; the probe is gone.

## What changes

- `pages.list` carries `mail_scout: bool` (false while pages itself is off,
  where the whole module is a 404 anyway).
- The reader's "Save to page" and a page's "Ask Scout" follow that flag, with
  no request of their own, and "Save to page" is offered in a workspace with
  no pages.
