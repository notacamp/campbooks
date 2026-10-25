## ADDED Requirements

### Requirement: Connecting a mailbox by its address
The Mailboxes page SHALL offer "Add a mailbox" (⇧C), a dialog that first asks
for the mailbox's address only and then asks `connectors.discover_imap` which
way it connects: a Gmail or Google Workspace address, or a Zoho one, SHALL be
offered its provider's sign-in (the front door, as Connect is) with "Also use
this account to sign in"; an Outlook, Hotmail, Live or Microsoft 365 address
SHALL get a note that Outlook is coming soon and nothing to connect; any other
address SHALL get the app-password step. When discovery answers
`provider_not_configured` the dialog SHALL offer the instance's mailbox
providers instead. The app-password step SHALL show a short how-to for the
provider, chosen from discovery's `provider_hint` or the address's domain:
Yahoo, AOL, iCloud, Fastmail, GMX and web.de (each saying to turn on IMAP in
its settings first), Yandex (also saying a new app password can take a few
hours to work), and a general text for any other domain; each named guide
SHALL link to the provider's own public help page, opened in a new tab. It
SHALL have one password field and an "Advanced" disclosure, closed unless no
settings were found, holding the username and the IMAP and SMTP host, port
and security, prefilled from discovery, where only TLS or STARTTLS can be
chosen. Connecting SHALL call `connectors.connect_imap` and SHALL explain
every refusal in plain words from its stable `code` (never the problem's
`title` or `detail`), with a general message for any other; a refusal with
`step_up_required` SHALL offer "Sign in again", as the page does. The
password SHALL never be logged, SHALL use `autocomplete="new-password"`, and
SHALL be cleared from the page's state once the request is made, on success
and when the dialog closes; it SHALL never be held in a cached mutation. The
dialog SHALL trap focus, close on Escape with nothing kept, submit each step
with Enter shown on its button, fit 375px and desktop, and be in en, pt-PT,
es-ES and fr.

#### Scenario: A Gmail address
- **WHEN** a member types `ana@gmail.com` and continues
- **THEN** the dialog offers "Connect Gmail" with "Also use this account to sign in", and pressing it starts `identity.start_link` with `connect` and `google`

#### Scenario: An Outlook address
- **WHEN** discovery answers `not_supported_yet` with `microsoft`
- **THEN** the dialog says Outlook is coming soon and offers nothing to connect

#### Scenario: A Yahoo address
- **WHEN** discovery answers `settings` with `provider_hint` `yahoo.com` and `password` `app_password`
- **THEN** the dialog shows Yahoo's app-password steps with a link to Yahoo's help page, one password field, and Advanced closed holding Yahoo's servers

#### Scenario: A GMX address
- **WHEN** discovery answers `settings` for `ana@gmx.net`
- **THEN** the how-to says to turn on IMAP in GMX's settings first and links to GMX's page for it

#### Scenario: Nothing found
- **WHEN** discovery answers `not_found`
- **THEN** the app-password step opens with Advanced open, its username prefilled with the address

#### Scenario: A wrong password at a provider that needs an app password
- **WHEN** `connectors.connect_imap` answers `imap_login_refused` for an address discovery said needs an app password
- **THEN** the dialog says the provider needs an app password, the password field is empty, and nothing of the server's text is shown

#### Scenario: The sign-in is too old
- **WHEN** discovery or connecting answers 403 `step_up_required`
- **THEN** the dialog says to sign in again first, and its button signs in again and comes back to the page

#### Scenario: IMAP turned off on the instance
- **WHEN** discovery answers 409 `provider_not_configured`
- **THEN** the dialog offers the instance's mailbox providers' sign-in buttons

### Requirement: An IMAP mailbox's address is checked automatically
When an IMAP mailbox is connected (or connected again) and its address is not
verified (its username is not its address), the module SHALL check, without
the member typing anything, that mail to the address reaches that mailbox:
it SHALL send one message to the address, from the address, through the
connection's own SMTP server (the submission, guard, TLS, budgets and session
slot of IMAP sending), carrying a code of at least 128 bits derived from the
instance's key, the connection, a fresh nonce and the address, in its
`Message-ID`. The check SHALL keep the address it checks, and SHALL only
ever verify that address: connecting the mailbox again with its address not
verified SHALL end every check under way (`address_changed`), whatever the
daily budget, and verifying SHALL require, in its transaction, that the
connection's address is still the checked one. The
message SHALL be plain text, say it comes from Campbooks and why, be written
in the member's locale, and carry `Auto-Submitted: auto-generated`. The
stream's runs SHALL then look for it in that connection's Inbox only, over
the run's own session after its pending changes and before its read, with a
`UID SEARCH` of the header and a fetch of each candidate's `Message-ID`, and
SHALL count only a message whose `Message-ID` is exactly the expected one.
Found within 15 minutes of the send, the connection's `address_verified`
SHALL become true in the transaction that ends the check. Past 15 minutes
(whether or not a run looked) or after 30 searches, the check SHALL end
`failed`, a code found later SHALL verify nothing, and sending SHALL stay
refused. The check holds only while the member cannot read the code: the
module SHALL not claim more (the member holds the IMAP password and can add
messages to the Inbox). While a check waits, the stream SHALL run at most the poll's
minimum interval apart. The code SHALL never be logged, put in a job
payload, an event or a job error. Each connection SHALL have at most 5
checks a day.

#### Scenario: The code comes back
- **WHEN** a mailbox signed in as `ana` with the address `ana@bakery.example` is connected, and the check message arrives in its Inbox
- **THEN** the next run finds it, the connection's `address_verified` is true, and a send from that inbox goes out

#### Scenario: The code never comes
- **WHEN** the check message never reaches the Inbox
- **THEN** 15 minutes after it was sent the check is `failed`, the address stays unverified, and a send from it fails `rejected`

#### Scenario: A server that lies
- **WHEN** the IMAP server answers every `UID SEARCH` with UIDs whose messages do not carry the expected `Message-ID`
- **THEN** nothing is verified

#### Scenario: The code of another check
- **WHEN** the Inbox holds a check message of an earlier check of the same connection, or of another connection
- **THEN** it does not verify the address

#### Scenario: The address changed after the checks were spent
- **WHEN** a member read the code of a connection's fifth check of the day, then connects the same mailbox again under another address and files that message in its Inbox
- **THEN** the check is `failed` (`address_changed`) and the new address is not verified

#### Scenario: A code for one address
- **WHEN** a check's message for `ana@bakery.example` is found after the connection's address became `billing@bakery.example`
- **THEN** nothing is verified

#### Scenario: Found too late
- **WHEN** no run looked for 16 minutes after the send and the message is then in the Inbox
- **THEN** the check is `failed` (`not_found`) and the address is not verified

#### Scenario: The server will not send as the address
- **WHEN** the SMTP server answers the check's `MAIL FROM` or `RCPT TO` with 550
- **THEN** the check is `failed` and nothing more is sent

#### Scenario: Too many checks
- **WHEN** a connection's address has been checked 5 times today
- **THEN** no more check message is sent until the budget refills

#### Scenario: Checks are isolated by workspace
- **WHEN** a transaction of one workspace reads the address checks
- **THEN** it sees none of another workspace's

### Requirement: The check message is removed safely
Once found, the check message SHALL be removed from the Inbox by exactly its
UID, and only with the write paths that cannot remove anything else: with
UIDPLUS, opened read-write, its `Message-ID` checked again, no other message
of the Inbox marked `\Deleted`, then `UID STORE +FLAGS.SILENT (\Deleted)` and
`UID EXPUNGE` of that UID (the mark taken off again if the expunge fails);
else, with MOVE, a `UID MOVE` of that UID to the Trash folder the stream
reads; else it SHALL be left in place. An answer that reports more than one
message expunged SHALL end the run as a hostile server. The module SHALL
never send `EXPUNGE` without UIDs, `CLOSE`, `DELETE` or `RENAME` for it, and
SHALL leave the Inbox with `UNSELECT` or `EXAMINE`.

#### Scenario: Expunged with UIDPLUS
- **WHEN** the check message is found on a server with UIDPLUS and nothing else in the Inbox is marked deleted
- **THEN** only its UID is marked and expunged, and every other message stays

#### Scenario: Another message marked deleted
- **WHEN** another message of the Inbox is already marked `\Deleted` and the server offers no MOVE
- **THEN** nothing is expunged and the check message stays, and the address is still verified

#### Scenario: Neither UIDPLUS nor MOVE
- **WHEN** the server offers neither
- **THEN** the check message is left in the Inbox and the address is verified

### Requirement: Checking an address again
`POST /api/v1/w/{workspace_id}/connections/{connection_id}/address-check`
(`connectors.check_address`, scope `connectors:write`, access Write on the
connection) SHALL start a new check of an IMAP connection whose address is
not verified and whose last check ended or is older than 30 minutes, with a
new nonce and the caller's locale, and answer 202 with the connection. It
SHALL answer 409 `address_check_not_needed` for a connection that is not
IMAP or is verified, 409 `needs_reauth` for one that needs the member, 404
for one the caller may not see, and 429 `rate_limited` past the daily
checks. `ConnectionView` SHALL carry `address_check`: `checking` while a check
is sending or waiting and younger than 30 minutes, `failed` once it ended
unverified (or went stale), and null when the address is verified or was
never checked.

#### Scenario: Try again
- **WHEN** the grantor asks to check a failed address again
- **THEN** the answer is 202 with `address_check` `checking`, and a new check message is sent

#### Scenario: Nothing to check
- **WHEN** the connection's address is verified
- **THEN** the answer is 409 `address_check_not_needed`

### Requirement: IMAP is on by default
`CAMPBOOKS_CONNECTORS__IMAP__ENABLED` SHALL default to true and
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED` to false. A server whose settings
name no IMAP variable SHALL start with any valid poll bounds. Without the
instance secret key, IMAP SHALL not be offered and the server SHALL start
(with a warning), unless `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` is set to true
explicitly, which SHALL stop it naming the missing key.

#### Scenario: Production's settings
- **WHEN** the server starts with `CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS=60`, `CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS=120`, the instance secret key and no IMAP variable
- **THEN** it starts, IMAP mailboxes can be connected, and IDLE is off

#### Scenario: No secret key
- **WHEN** an instance with no instance secret key and no IMAP variable starts
- **THEN** it starts and IMAP is not offered

#### Scenario: Asked for without a key
- **WHEN** an instance sets `CAMPBOOKS_CONNECTORS__IMAP__ENABLED=true` and has no instance secret key
- **THEN** it does not start, and the error names `CAMPBOOKS_SECRET_KEY`

## MODIFIED Requirements

### Requirement: Mailboxes screen
Settings SHALL have a Mailboxes page listing the workspace's connections the
person may see (`connectors.list_connections`), each with its provider, its
address, who connected it ("you", from the server's `granted_by_you`, never
by matching an email, or the member's name) and its health in
plain words: `ok` "Syncing", `needs_reauth` "Needs you to sign in again"
(or the grantor's name), `throttled` "Paused by <provider> for a few
minutes", `failing` "Not syncing — we'll keep trying". An IMAP mailbox whose
address is not verified SHALL also say so: "Checking your address…" while
`address_check` is `checking`, and "We couldn't verify this address: sending
stays off" with "Try again" (`connectors.check_address`, the grantor only)
when it is `failed` or was never checked, so the member knows sending is not
available from it. The page SHALL offer:

- "Add a mailbox" (the Connecting a mailbox by its address requirement),
  whose provider sign-ins go through `identity.start_link` with `connect` and
  this page as `return_to`, asking "Also use this account to sign in"
  (ticked by default, untick it for a shared mailbox), passed as `sign_in`;
- Reconnect, on the person's own connections that need re-authorisation,
  through `identity.start_link` with `reconnect`;
- Disconnect (`connectors.disconnect`), only on the person's own
  connections, behind an inline confirmation saying that syncing stops and
  access is given up at the provider, asking what becomes of its mail in
  Campbooks ("Delete its mail from Campbooks", the default, or "Keep its
  history, read-only"), that the mail stays at the provider either way, and
  what becomes of the way in: that it goes too only when `identity.list_ways_in`
  records this very connection as the only mailbox of a way in a connect
  created and it is not the last way in, and otherwise that how the person
  signs in does not change (never decided from an address), and nothing
  until the ways in and passkeys are known. Its cancel SHALL read "Don't
  disconnect". A disconnect that fails SHALL be said as a failure, never
  shown as done; one that finds nothing (404) SHALL be said by the page, as
  the row leaves with the list's next answer; focus
  SHALL return to Disconnect when the confirmation closes and the row stays,
  and move to the result once it is gone;
- "Sign in again" (a fresh sign-in, back to this page) when starting is
  refused with `step_up_required`.

Right after an IMAP mailbox is connected, the dialog SHALL say it is
connected, and, when its address is not verified, show "Checking your
address…" until the connection's `address_check` changes, then "Verified" or
"We couldn't verify this address: sending stays off" with "Try again".

Back from the front door, the page SHALL say once that the mailbox is
connected (`connected=`) or explain the refusal (`link_error=`) for every
code the front door and the grant intake return, with a generic message for
any other, and SHALL clear the query. Keys: ⇧C opens Add a mailbox, J and K
choose a mailbox, R reconnects it and ⌫ asks to disconnect it, each shown on
its button. The page SHALL be in en, pt-PT, es-ES and fr, show loading,
empty and error states, fit 375px, and pass axe in light and dark.

#### Scenario: Connect Zoho Mail
- **WHEN** the server lists `google` and `zoho` as mailbox providers, the person adds `ana@zohomail.com`, which discovery routes to Zoho, and presses "Connect Zoho Mail"
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
- **WHEN** the person opens the disconnect confirmation of a connection that `identity.list_ways_in` records as the only mailbox of a way in a connect created, and they have another way in
- **THEN** the confirmation says it also stops being a way to sign in

#### Scenario: A disconnect that finds nothing
- **WHEN** `connectors.disconnect` answers 404 and the list no longer has the connection
- **THEN** the row goes, an alert on the page says the mailbox was not found, and nothing says it is disconnected

#### Scenario: Keep the history
- **WHEN** the person chooses "Keep its history, read-only" and confirms
- **THEN** `connectors.disconnect` is called with `mail=keep`

#### Scenario: A colleague's mailbox
- **WHEN** an owner sees a member's connection
- **THEN** the row offers no Disconnect

#### Scenario: A shared mailbox
- **WHEN** a member opens the disconnect confirmation of a mailbox they connected without signing in with it
- **THEN** both choices are offered, and the confirmation says how they sign in does not change

#### Scenario: A colleague's mailbox needs re-authorisation
- **WHEN** an owner sees a member's connection with health `needs_reauth`
- **THEN** the row names the member who must sign in again and offers no Reconnect

#### Scenario: An address that could not be checked
- **WHEN** the person's IMAP mailbox has `address_check` `failed`
- **THEN** its row says sending stays off until the address is verified and offers "Try again", which calls `connectors.check_address`

### Requirement: Connecting an IMAP mailbox
`connectors.connect_imap` SHALL connect an IMAP mailbox only after signing
in to its IMAP server and authenticating to its SMTP server with the given
password, over TLS or STARTTLS with verified certificates, and SHALL keep
nothing when either fails. It SHALL require a member token, `connectors:write`,
permission to create a connection, and a sign-in within the last ten
minutes. Refusals SHALL name what went wrong with a stable code and SHALL
never repeat the server's own text. The connection SHALL carry the
`messages` capability and SHALL never be a way to sign in. A connection whose
address is not verified SHALL start its address check. Reconnecting a
connection whose address was verified, with the same address, SHALL keep it
verified.

#### Scenario: App password accepted
- **WHEN** a member who signed in five minutes ago connects `ana@yahoo.com` with a valid app password
- **THEN** the connection is created with provider `imap`, its `messages` stream starts, and `ConnectionAdded` is published

#### Scenario: Wrong password
- **WHEN** the IMAP server refuses the login
- **THEN** the answer is `imap_login_refused`, nothing is stored, and for Yahoo it adds that an app password is needed

#### Scenario: Server without TLS
- **WHEN** the IMAP server on port 143 does not offer STARTTLS
- **THEN** the answer is `starttls_missing` and no password was sent

#### Scenario: Old sign-in
- **WHEN** the member signed in more than ten minutes ago
- **THEN** the answer is 403 `step_up_required`

#### Scenario: Client token
- **WHEN** an API client token calls the operation
- **THEN** the answer is `connection_needs_user`

#### Scenario: Reconnecting with a new password
- **WHEN** the member who connected a mailbox reconnects it with a new valid password
- **THEN** the same connection keeps its streams, the new password replaces the old one, and its health is `ok`

#### Scenario: A username that is not the address
- **WHEN** a member connects `ana@bakery.example` signing in as `ana`
- **THEN** the connection's `address_verified` is false, its `address_check` is `checking`, and a check message is queued

#### Scenario: Turned off
- **WHEN** `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` is false
- **THEN** both IMAP operations answer `provider_not_configured` and `imap` is not offered

### Requirement: Zoho Mail connections in their data centre
The module SHALL connect Zoho Mail accounts when `CAMPBOOKS_ZOHO__CLIENT_ID`
and a client secret are set, asking in the grant for
`ZohoMail.accounts.READ`, `ZohoMail.folders.READ`, `ZohoMail.messages.ALL`
and `ZohoMail.tags.ALL` (the member's labels and the stream marks; a
connection granted without it keeps syncing without labels until the member
connects again), and, while `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on,
`ZohoCalendar.calendar.READ` and `ZohoCalendar.event.ALL`.
A Zoho grant SHALL be taken only when its issuer is one of Zoho's accounts
servers (`.com`, `.eu`, `.in`, `.com.au`, `.jp`, `.ca`, `.sa`, `.uk`), or, in
development only, the server `CAMPBOOKS_ZOHO__BASE_URL` names; any other
SHALL be refused with `unknown_data_centre`. Its tokens SHALL be refreshed
and revoked at that accounts server (the token in the body first, the
query only if Zoho refuses that; a refusal is never taken as revoked), and its mail read and written at that
data centre's mail host (`accounts.zoho.<x>` → `mail.zoho.<x>`), never at a
host taken from a request or a provider's answer. A refresh Zoho refuses
(`invalid_code` or `invalid_grant`, even in a 200 answer) SHALL set
`needs_reauth`.

#### Scenario: A European account
- **WHEN** a grant issued by `https://accounts.zoho.eu` is connected and its stream runs
- **THEN** its token is refreshed at `https://accounts.zoho.eu/oauth/v2/token` and its mail is read from `https://mail.zoho.eu/api/`

#### Scenario: A made-up data centre
- **WHEN** a Zoho grant names the issuer `https://accounts.evil.example`
- **THEN** no connection is created and the answer is `unknown_data_centre`
