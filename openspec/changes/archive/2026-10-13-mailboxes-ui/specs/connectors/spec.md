## ADDED Requirements

### Requirement: Mailboxes screen
Settings SHALL have a Mailboxes page listing the workspace's connections the
person may see (`connectors.list_connections`), each with its provider, its
address, who connected it ("you" or the member's name) and its health in
plain words: `ok` "Syncing", `needs_reauth` "Needs you to sign in again"
(or the grantor's name), `throttled` "Paused by <provider> for a few
minutes", `failing` "Not syncing — we'll keep trying". The page SHALL offer:

- Connect for each of the server's `mailbox_providers`, through
  `identity.start_link` with `connect` and this page as `return_to`, asking
  "Also use this account to sign in" (ticked by default, untick it for a
  shared mailbox), passed as `sign_in`;
- Reconnect, on the person's own connections that need re-authorisation,
  through `identity.start_link` with `reconnect`;
- Disconnect (`connectors.disconnect`), only on the person's own
  connections, behind an inline confirmation saying that syncing stops and
  access is given up at the provider, asking what becomes of its mail in
  Campbooks ("Delete its mail from Campbooks", the default, or "Keep its
  history, read-only" where `keep` is allowed), that the mail stays at the
  provider either way, and whether signing in with that account keeps
  working;
- "Sign in again" (a fresh sign-in, back to this page) when starting is
  refused with `step_up_required`.

Back from the front door, the page SHALL say once that the mailbox is
connected (`connected=`) or explain the refusal (`link_error=`) for every
code the front door and the grant intake return, with a generic message for
any other, and SHALL clear the query. Keys: ⇧C focuses Connect, J and K
choose a mailbox, R reconnects it and ⌫ asks to disconnect it, each shown on
its button. The page SHALL be in en, pt-PT, es-ES and fr, show loading,
empty and error states, fit 375px, and pass axe in light and dark.

#### Scenario: Connect Zoho Mail
- **WHEN** the server lists `google` and `zoho` as mailbox providers and the person presses "Connect Zoho Mail"
- **THEN** `identity.start_link` is called with `connect`, `zoho`, the workspace and this page as `return_to`, and the browser goes to the URL it returns

#### Scenario: Back connected
- **WHEN** the page opens with `?connected=<id>` and the list has that connection
- **THEN** a status says the mailbox is connected, the list shows it as Syncing, and the address bar loses the query

#### Scenario: Refusal explained
- **WHEN** the page opens with `?link_error=account_connected_by_another_member`
- **THEN** an alert explains that another member already connected that mailbox here

#### Scenario: Last sign-in too old
- **WHEN** starting a connect is refused with 403 `step_up_required`
- **THEN** "Sign in again first" is shown, and its button signs in again with `max_age=600` and comes back to the page

#### Scenario: Disconnect deletes the mail by default and keeps the way in
- **WHEN** the person disconnects their own mailbox and confirms without changing the choice
- **THEN** `connectors.disconnect` is called with `mail=delete`, the row is gone, and the person's ways in are unchanged

#### Scenario: Keep the history
- **WHEN** the person chooses "Keep its history, read-only" and confirms
- **THEN** `connectors.disconnect` is called with `mail=keep`

#### Scenario: A colleague's mailbox
- **WHEN** an owner sees a member's connection
- **THEN** the row offers no Disconnect

#### Scenario: A shared mailbox
- **WHEN** a member opens the disconnect confirmation of a mailbox they connected without signing in with it
- **THEN** only deleting its mail is offered, and the confirmation says it is not one of their ways to sign in

#### Scenario: A colleague's mailbox needs re-authorisation
- **WHEN** an owner sees a member's connection with health `needs_reauth`
- **THEN** the row names the member who must sign in again and offers no Reconnect

## MODIFIED Requirements

### Requirement: Disconnecting
Only the member who granted a connection SHALL be able to disconnect it;
an owner or admin who can see another member's connection SHALL get 403
`not_your_connection`. The caller SHALL say what becomes of its mail in
Campbooks with `mail`: `delete` (the default) or `keep` (read-only
history). `keep` SHALL be allowed only when the grantor signs in with the
account (`grantor_signs_in`) or is an owner or admin, and refused otherwise
with 403 `keep_mail_not_allowed`. The grant SHALL be revoked at the
provider, credentials, streams and indexes deleted, and
`connectors.connection_removed` published with the inbox ids and the
`mail` choice, for the inboxes' owner to carry out. When a member leaves a workspace, the connections they granted
there SHALL be disconnected the same way. When a workspace is being
deleted, every grant SHALL be revoked and every connection deleted the same
way at once.

#### Scenario: Grantor disconnects
- **WHEN** the grantor disconnects a Google connection
- **THEN** Google's revoke endpoint is called, the connection is gone, and `connectors.connection_removed` lists its inbox id

#### Scenario: An admin cannot disconnect a member's mailbox
- **WHEN** an admin disconnects a connection a member granted
- **THEN** the response is 403 `not_your_connection` and nothing is revoked

#### Scenario: Keeping a shared mailbox's mail
- **WHEN** a member disconnects a shared mailbox they connected without signing in with it, asking `mail=keep`
- **THEN** the response is 403 `keep_mail_not_allowed`; with `mail=delete` the event carries `mail` = `delete`

#### Scenario: Grantor leaves the workspace
- **WHEN** `workspaces.member_left` arrives for a member who granted a connection
- **THEN** that connection is disconnected and revoked

#### Scenario: Workspace deleted
- **WHEN** `workspaces.workspace_deleting` arrives for a workspace with a connection
- **THEN** its grant is revoked and the connection is gone, without waiting for the grace period
