# Proposal

## Why

The owner signs up with Google, and that one mailbox connects. There is no
screen to connect a second one (a Zoho Mail account, say), to see whether a
mailbox is still syncing, to reconnect one that stopped, or to disconnect
one. Home's "Connect your mail" button has been disabled since the first-run
screens shipped. The server has had everything needed since the ways-in
slice: `identity.start_link` with the `connect` and `reconnect` purposes,
`connectors.list_connections` with health, and `connectors.disconnect`.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/68

This change adds **Settings → Mailboxes** and connects Home to it:

- the workspace's connected mailboxes, each with its provider, address, who
  connected it, and its health in plain words ("Syncing", "Needs you to sign
  in again", "Paused by Gmail for a few minutes", "Not syncing — we'll keep
  trying");
- **Connect a mailbox** for each provider this instance can connect mail
  from, through `identity.start_link` (`connect`), coming back to the page
  with a success or an explained refusal; "Sign in again" when the last
  sign-in is too old;
- **Reconnect** for a mailbox of yours that needs it (`reconnect`);
- **Disconnect**, only for the person who connected the mailbox (an owner
  decision from the security audit, campbooks-cloud#117), behind an inline
  confirmation that asks what happens to its mail in Campbooks (delete by
  default, or keep its history read-only where allowed), says the mail
  stays at the provider either way, and says whether signing in with that
  account keeps working;
- Connect asks **"Also use this account to sign in"** (ticked by default;
  untick it for a shared mailbox, which then never becomes a way in);
- keyboard: ⇧C connect, J/K choose a mailbox, R reconnect it, ⌫ disconnect
  it (confirmed in place), all shown on the buttons;
- Home's "Connect your mail" opens the page, and the mail section links to
  it once mailboxes are connected or while the sign-up mailbox waits;
- settings sections (Members, Mailboxes, Ways to sign in) in the sidebar on
  desktop and as chips on phones.

**Modules touched**
- **identity** (server): `identity.list_ways_in` also lists the providers a
  mailbox can be connected from (`mailbox_providers`); `identity.start_link`
  takes `sign_in` for a connect (migration `identity__connect_without_way_in`).
- **kernel**: `ProviderGrant.signs_in`.
- **connectors** (server): `ConnectionView.grantor_signs_in` (migration
  `connectors__grantor_signs_in`); `connectors.disconnect` is grantor-only
  and takes `mail=delete|keep`, published on `connectors.connection_removed`.
- **clients/app**:
  - `connectors`: the Mailboxes page, the disconnect mutation, Home's
    mailboxes section linking to the page;
  - `identity`: `useMailboxConsent`, the client side of the front door
    (start connect/reconnect, step-up), handed to `connectors` by the app
    shell, as the server registers `identity`'s consent door for
    `connectors`;
  - app shell: route wiring, settings sections, Home's button;
  - `lib/ui`: `ProviderMark`, `HealthText`, `PhoneSubnav`, and a selected
    state for `ListRow`.
- **lib/shortcuts**: a `settings` scope for keys that only settings pages
  bind.

No new module and no new cross-module dependency on the server. On the
client, `connectors` imports `workspaces` (members, to say who connected a
mailbox) through its `index.ts`; `identity` already imports `connectors`.

## Non-goals

- Calendar, files or bank capabilities: the page connects mail only.
- Microsoft mailboxes: there is no Microsoft mail adapter yet; the page
  offers only what the server lists.
- Carrying out the purge of a disconnected mailbox's mail: the conversations
  module's, in batch 1 of campbooks-cloud#117; this change defines the
  contract (`mail` on the operation and the event).
- Removing a way in that came only from a connect when its connection goes
  (campbooks-cloud#117).
- Undo for Disconnect: the grant is revoked at the provider, so only a new
  consent brings it back.
- Choosing which of a Zoho account's several mailboxes sync; per-stream
  pause; sync-now from the UI.
- Changing `connectors.start_connection` (it keeps returning to `/`).
