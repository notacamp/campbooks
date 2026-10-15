## ADDED Requirements

### Requirement: Mailboxes screen
Settings SHALL have a Mailboxes page listing the workspace's connections the
person may see (`connectors.list_connections`), each with its provider, its
address, who connected it ("you", from the server's `granted_by_you`, never
by matching an email, or the member's name) and its health in
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
  history, read-only"), that the mail stays at the provider either way, and
  what becomes of the way in: it goes too when it was added only by
  connecting the mailbox and is not the last way in, it stays otherwise, and
  a mailbox that is not a way in changes nothing there. A disconnect that
  fails, including one that finds nothing (404), SHALL be said as a failure,
  never shown as done;
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

#### Scenario: Disconnect deletes the mail by default
- **WHEN** the person disconnects their own mailbox and confirms without changing the choice
- **THEN** `connectors.disconnect` is called with `mail=delete` and the row is gone

#### Scenario: The way in goes with a connect-only mailbox
- **WHEN** the person opens the disconnect confirmation of a mailbox whose account became a way in only by connecting it, and they have another way in
- **THEN** the confirmation says it also stops being a way to sign in

#### Scenario: A disconnect that finds nothing
- **WHEN** `connectors.disconnect` answers 404
- **THEN** an alert says the mailbox was not found, and nothing says it is disconnected

#### Scenario: Keep the history
- **WHEN** the person chooses "Keep its history, read-only" and confirms
- **THEN** `connectors.disconnect` is called with `mail=keep`

#### Scenario: A colleague's mailbox
- **WHEN** an owner sees a member's connection
- **THEN** the row offers no Disconnect

#### Scenario: A shared mailbox
- **WHEN** a member opens the disconnect confirmation of a mailbox they connected without signing in with it
- **THEN** both choices are offered, and the confirmation says it is not one of their ways to sign in

#### Scenario: A colleague's mailbox needs re-authorisation
- **WHEN** an owner sees a member's connection with health `needs_reauth`
- **THEN** the row names the member who must sign in again and offers no Reconnect
