# Design

## Context

Since the ways-in slice (campbooks-cloud#79), `identity` is the one front
door for provider consent (ADR 0014): `identity.start_link` issues a
single-use ticket for `link`, `connect` or `reconnect`, the browser goes to
`/auth/start/{provider}?link=…`, and the round trip ends back on the
caller's `return_to` with `connected=<connection id>` or
`link_error=<code>`. Connecting needs a sign-in in the last ten minutes
(`step_up_required`); reconnecting a grant that stopped working does not.
`connectors.list_connections` returns each connection with its health
(`ok`, `needs_reauth`, `throttled`, `failing`); `connectors.disconnect`
revokes the grant at the provider and deletes the connection.

What was missing is the screen. This document decides where it lives, how a
client module reaches the front door without a dependency cycle, the one
server read the screen needs, what Disconnect says, and the keys.

## Where the screen lives

### Alternatives considered

1. **Its own page, Settings → Mailboxes, in the `connectors` client module
   (chosen).** Mailboxes are the workspace's: owners and admins see every
   member's, members see their own, and any of them may be disconnected by
   an owner. Ways in are the person's, across every workspace. A page per
   concept keeps each rule in one place: removing a way in never
   disconnects a mailbox, and disconnecting a mailbox never removes a way
   in. The client module mirrors the server module that owns the data
   (`connectors`).
2. **A section of Ways to sign in.** Fewer pages, and a connected mailbox
   *is* a way in (ADR 0014). But the section would show colleagues'
   mailboxes on a page about your own account, Remove and Disconnect would
   sit next to each other with opposite consequences, and the page would
   change with the open workspace while the rest of it does not. Rejected.
3. **The page in the `identity` client module**, next to Ways to sign in,
   using `identity`'s hooks directly. Least plumbing, but the page's data,
   rules and mutations are `connectors`', and the client modules are meant
   to mirror the server's crates. Rejected.

The old Rails app on `main` has an "email accounts" settings page too; this
design does not copy it. It is chosen for the reasons above (data owner,
the scope difference between workspace mailboxes and personal ways in),
and the flow underneath (one front door, `return_to`, refusal codes) is
Next's own.

## Reaching the front door from `connectors`

`identity`'s client already imports `connectors` (Ways to sign in shows a
Reconnect for your own mailboxes), and dependency-cruiser forbids cycles, so
`connectors` cannot import `identity`.

### Alternatives considered

1. **A port, filled by the app shell (chosen).** `connectors` declares
   `UseMailboxConsent`, a hook type: the providers a mailbox can be
   connected from, `start({ connect | reconnect })`, whether it is starting,
   why it was refused, and "sign in again". `identity` implements it
   (`useMailboxConsent`, over `identity.start_link` and its step-up URL);
   the app's module manifest hands it to `buildConnectorsRoutes`. This is
   the client mirror of the server, where `identity` implements the
   kernel's `ConsentDoor` and the server registers it for `connectors`.
   The same pattern already hands `identity`'s `MailboxStep` to the
   `workspaces` routes. No new machinery, no cycle.
2. **Call `connectors.start_connection` from `connectors`.** No port
   needed: the server already delegates it to the front door. But the
   kernel's `ConsentRequest` has no `return_to`, so the browser would come
   back to `/`, not to the page; fixing that touches the kernel port, both
   modules and the API contract, and the operation speaks of capabilities
   the page never offers. Rejected for now; it stays for API clients.
3. **Move the shared pieces to `lib/`.** `lib/` is business-agnostic and
   may not import modules; the front door's purposes and step-up rules are
   `identity`'s. Rejected.

## The providers on offer

The page must offer "only the providers the server lists". The server lists
sign-in providers (`identity.list_ways_in` → `providers`), but connecting a
mailbox also needs a mail adapter for that provider (`GrantIntake::
mailbox_scopes` non-empty): Microsoft signs in but has no mail adapter.

### Alternatives considered

1. **A `mailbox_providers` field on `identity.list_ways_in` (chosen).**
   `identity` decides what `start_link(connect)` accepts (configured
   upstream *and* mailbox scopes), so the same module says what is on
   offer, from the same two checks, in display order. One field on an
   existing read: no new operation, scope or migration. The response is
   already per caller and needs `identity:read`, which every web session
   has.
2. **A new operation `identity.list_mailbox_providers`.** Cleaner name,
   but one more endpoint, OpenAPI entry and request on the page for a list
   of at most three words. Rejected.
3. **A list from `connectors`.** `connectors` knows its adapters but not
   whether `identity` has the provider configured for sign-in; it would
   have to ask through a new kernel port. Rejected.

## Data ownership, events, scopes

- Schema `identity`: `link_tickets.adds_way_in` and `upstream_flows.adds_way_in`
  (migration `20261020140000_identity__connect_without_way_in`), and the
  new `identity.mailbox_links` (`20261020140100_identity__mailbox_links`).
- Consumed: `connectors.connection_removed` v1 by
  `identity.forget_connected_way_in` (reads `workspace`, `provider` and `account_subject`).
  Published as before: `identity.way_in_removed` when a way in goes with it.
- No other schema changes; the disconnect side is batch 1's.

| Operation | Scope | Used for |
|---|---|---|
| `connectors.list_connections` | `connectors:read` | the list and health |
| `connectors.disconnect` | `connectors:write` | Disconnect (`?mail=delete|keep`, grantor only) |
| `identity.list_ways_in` | `identity:read` | `mailbox_providers` |
| `identity.start_link` | `identity:write` | Connect (`connect`), Reconnect (`reconnect`) |
| `workspaces.list_members` | `workspaces:read` | "Connected by" |

A connection the caller may not see is 404 everywhere, as before; the page
only lists what `list_connections` returns.

## The page

- **Rows**: provider mark and name, the address, "Connected by you" or by a
  member's name, and the health line:
  - `ok`: "Syncing";
  - `needs_reauth`: "Needs you to sign in again" (yours), or "Needs Ana
    Duarte to sign in again" (a colleague's, which only its grantor can
    reconnect: `reconnect_target` is 404 for anyone else);
  - `throttled`: "Paused by Gmail for a few minutes" (or Zoho Mail);
  - `failing`: "Not syncing — we'll keep trying".
- **Reconnect** shows only on your own mailbox that needs it.
- **Disconnect** opens an inline confirmation: "Campbooks stops syncing
  this mailbox and gives up its access at Gmail. The mail already in
  Campbooks stays, and you can still sign in with this account." After it,
  a status banner says it is disconnected. No Undo: the grant is revoked at
  the provider, so only a new consent brings it back; the page says
  Connect is how.
- **Connect a mailbox**: one button per `mailbox_providers` entry ("Connect
  Gmail", "Connect Zoho Mail"); with none, a sentence saying this instance
  has no mail provider set up.
- **Coming back**: `?connected=<id>` → "Zoho Mail is connected", naming the
  address once the list has it; `?link_error=<code>` → a title and a lead
  for every code the front door and the grant intake send back:
  `sign_in_again`, `link_expired`, `cancelled_at_provider`, `login_failed`,
  `provider_unavailable`, `consent_incomplete`, `account_mismatch`,
  `account_connected_by_another_member`,
  `way_in_belongs_to_another_account`, `mailbox_unsupported`,
  `provider_not_configured`, `no_offline_access`, `unknown_data_centre`,
  `no_mailbox`, and anything else as a generic one. The query is read once
  and cleared, as on Ways to sign in.
- **Starting refused** (the API call itself): `step_up_required` → the
  "Sign in again first" banner reused from Ways to sign in (its button goes
  to `/bff/login?max_age=600` and back here); `mailbox_unsupported`,
  `rate_limited` and anything else → a sentence each.

## Keys

`c` is global (new message) and `r`, `⇧R` belong to the reader, and the
registry refuses keys that two live scopes share. The page's keys get a new
`settings` scope that overlaps only `global` (and itself): a settings page
never shows the inbox's list, item or composer.

| Key | Does |
|---|---|
| ⇧C | Focus the first Connect button (Enter starts it) |
| J / K | Choose the next / previous mailbox (existing `list.next` / `list.previous`) |
| R | Reconnect the chosen mailbox, when it needs you |
| ⌫ | Disconnect the chosen mailbox: opens the inline confirmation (focus on Cancel, Esc cancels) |

The chosen row is highlighted, and its buttons carry the hints. The first
choice is the first mailbox that needs you, else the first.

## Settings sections

Settings had one page in the sidebar (Members) and Ways to sign in behind
the account menu. Inside Settings, the sidebar now nests Members,
Mailboxes and Ways to sign in under Settings (as the streams nest under
Inbox), and phones get the same three as chips under the header. Home's
"Connect your mail" opens Mailboxes; Home's mail section links to it once
mailboxes are connected, and the held-mailbox offer offers "Connect a
different mailbox".

## Owner decisions from the security audit (campbooks-cloud#117)

Three decisions arrived during this change and are built here.

1. **Connect asks "Also use this account to sign in".** `identity.start_link`
   gains `sign_in` (connect only; omitted means `true`, as before, so
   `connectors.start_connection` and API callers are unchanged). With
   `false`, the callback links nothing: an identity that is another
   account's way in is still refused, one that already is the person's way
   in counts as theirs, and an unknown one stays unlinked. The choice rides
   on the link ticket and the upstream flow (`adds_way_in`, migration
   `identity__connect_without_way_in`). The grant crosses to `connectors`
   with a new `ProviderGrant.signs_in`, stored as
   `connectors.connections.grantor_signs_in` (migration
   `connectors__grantor_signs_in`; existing rows `true`: every earlier
   connection came with a way in). A later grant only ever sets it (a
   reconnect passes `false` and changes nothing). The box is ticked by
   default: before the provider round trip nobody knows which account will
   come back, so the page defaults to the common case (your own mailbox)
   and says when to untick it.
   - *Alternative considered:* a new purpose `connect_shared` instead of a
     flag. Rejected: it multiplies purposes, check constraints and branches
     for one boolean.
2. **Only the grantor disconnects.** `connectors.disconnect` answers 403
   `not_your_connection` to an owner or admin who can see another member's
   connection (still 404 for one they cannot see). The page shows
   Disconnect, and binds ⌫, only on the person's own rows. Member
   departure and workspace deletion still disconnect on their own (no
   caller).
3. **Disconnect asks what happens to the mail.** `connectors.disconnect`
   takes `?mail=delete|keep` (default `delete`) and publishes it on
   `connectors.connection_removed` (`mail`, `#[serde(default)]`, so events
   recorded before it read as `delete`; version unchanged because the field
   is additive). `keep` is allowed only when the grantor signs in with the
   account (their own mailbox) or is an owner or admin (403
   `keep_mail_not_allowed` otherwise). The page offers both as radio cards,
   delete preselected; it offers only delete when keep is not allowed. It
   says the mail stays at the provider either way, and whether signing in
   with that account keeps working. **The purge itself is the conversations
   module's** and is being built in batch 1 of campbooks-cloud#117: it acts
   on `connection_removed` with `mail = delete`. Until it lands, nothing
   consumes the event and the mail stays in Campbooks whatever the choice.
   The confirmation describes what each choice will do.
   - **Superseded by batch 1 of #117** (its disconnect backend replaces this
     one when it lands): only the grantor disconnects (403
     `disconnect_owner_only`), so keeping the history is always the
     grantor's choice; owners and admins keep a leaver's mail at offboarding
     (`DELETE …/members/{user}?mail=keep`). The page follows that shape now.
   - *Alternative considered:* a separate `connectors.purge_mail` operation
     after a disconnect. Rejected: two calls for one decision, and a window
     where a disconnected mailbox's mail has no stated fate.

Member departure and workspace deletion publish `mail = delete`.

## After batch 1 of #117 landed (#479)

Batch 1 brought the disconnect backend this change had started: only the
grantor disconnects (403 `disconnect_owner_only`), `?mail=delete|keep`,
`connectors.connection_removed` v1 with `granted_by`, `provider`,
`account_subject` and `mail`, the conversations consumer that deletes or
freezes the inboxes, and `list_inboxes` `read_only`. This change dropped its
own versions (and `grantor_signs_in`, which nothing decided from any more)
and keeps the screen on top of that API, plus:

- **The reader for kept inboxes** (inbox-ui): an inbox listed `read_only`, or
  one the person only reads, opens with no write action, is never marked
  read on open, and says "Kept as history". New messages never offer it.
- **The way in goes with its mailbox** (identity), below.
- **"You" from the server:** `ConnectionView.granted_by_you`,
  `MemberView.you`.

### Removing a connect-only way in

The rule (approved #117): a way in that came only from connecting a mailbox
goes when that mailbox is disconnected, unless another live connection uses
it or it is the account's last way in. The hard part is "another live
connection": `identity` cannot see `connectors`' connections, which live in
per-workspace rows behind row-level security.

*Alternatives considered:*

1. **An identity-owned record of the workspaces a way in's mailbox is
   connected in (chosen).** `identity.mailbox_links (provider_identity_id,
   in_workspace_id, connection_id)`. The connectors module keeps one
   connection per workspace, provider and account, so (way in, workspace)
   names the connection before it exists. That lets the row be written **in
   the transaction that creates the way in**, or before a later grant is
   handed over: if it cannot be written, the round trip fails, and a way in
   a connect created is never left untracked. The connection id is filled in
   afterwards, only to show which mailbox a way in came with
   (`list_ways_in`). Account data, like `consents.bound_workspace_id`: no
   row-level security, so "still connected elsewhere" sees every
   workspace. The consumer reads the event's workspace, `provider` and
   `account_subject`, **locks the account first** (as removing a way in on
   Ways to sign in does, so the two never deadlock, and two of its mailboxes
   removed at once are counted one after the other), then deletes the row
   and counts the others. A way in linked before the table existed is never
   tracked, so it is never removed by mistake.
2. **Ask `connectors` through a kernel port** ("does any connection of
   provider X, account S remain?"). A cross-workspace read of `connectors`'
   rows: either a privileged connection that skips row-level security or a
   new owner-less query path, both against the data-access rules #117
   tightened. Rejected.
3. **Match by `provider` + `account_subject` from the event and count
   nothing.** Removes the way in on the first disconnect even while another
   workspace's connection still uses it. Rejected.

### The sign-in allow-list (#495)

The allow-list gates every door that creates or uses a way in. A connect
with `sign_in: true` (and a link) is checked before the transaction that
links the way in and records its mailbox, so a refusal writes nothing and
hands nothing over. A connect with `sign_in: false` and a reconnect are not
checked: they never create a way in or a session, so they are no road around
the list (signing in with that account is still refused), and the list is no
road around them either (a shared mailbox's content is the workspace's
choice, not a sign-in).

## Risks

- **Mail after Disconnect.** Until #117's purge consumes `mail = delete`,
  a deleted mailbox's conversations stay. The contract (`mail` on the
  operation and the event) is what the two pieces agree on.
- **What the confirmation says about the way in** comes from the server's
  record only: `list_ways_in` gives each way in its recorded mailboxes and
  the connections among them. It says the way in goes only when this
  connection is the only one recorded for a way in a connect created and it
  is not the last way in; otherwise that how you sign in does not change.
  No address is ever matched.
- **`grantor_signs_in` was dropped.** A first version stored whether the
  grantor signed in with the account, to limit who could keep the mail. It
  went stale as soon as a way in was removed, and batch 1's rule (the
  grantor always chooses) made it unnecessary.
- **Who "you" are** comes from the server (`ConnectionView.granted_by_you`,
  `MemberView.you`), never from matching emails, which a person without an
  address (or two sharing one) would get wrong.
- **Revoking at Google revokes the person's whole grant to the app**, which
  may include a sign-in's consent; signing in again simply asks again, so
  the way in still works, as the confirmation promises.
