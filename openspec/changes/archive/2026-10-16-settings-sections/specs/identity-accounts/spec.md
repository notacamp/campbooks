# identity-accounts Specification

## MODIFIED Requirements

### Requirement: Ways to sign in screen
Settings SHALL have a Ways to sign in page, made of section cards
(`settings-ui`) under the lead "How you get into Campbooks. Keep at least two,
so losing one never locks you out.": an "Accounts" card with a count listing
the person's provider ways in, with Remove behind an inline confirmation (and
`last_way_in` explained in place), Reconnect for this workspace's connections
of theirs that need re-authorisation, and, at its bottom, Add for each
configured provider; further sections other slices contribute (passkeys); a
"Sign in again" offer when an action needs a recent sign-in; a notice and a
"New" mark when a way in (other than the sign-up's) was added in the last
seven days; and Sign out everywhere as its own card, its button outlined in
the danger tone, behind an inline confirmation. The sign-in screen SHALL offer
every provider the server lists. Both SHALL be in en, pt-PT, es-ES and fr,
keyboard-operable with visible hints, and usable at 375px.

#### Scenario: Remove with confirmation
- **WHEN** the person presses Remove on a way in
- **THEN** an inline confirmation names the provider and address and says the mailbox stays connected, and nothing is removed until they confirm

#### Scenario: Microsoft and Zoho offered
- **WHEN** the server lists `google`, `microsoft` and `zoho` for a sign-in request
- **THEN** the sign-in screen shows the three, in that order, with `1`, `2` and `3`

#### Scenario: Added this week
- **WHEN** a Zoho account was added two days ago
- **THEN** its row is marked New and the page says to remove it and sign out everywhere if it wasn't the person

### Requirement: Passkeys section
Settings → Ways to sign in SHALL have a Passkeys card below the Accounts card:
the person's passkeys (synced or on one device, added, last used), Rename in
place, Remove behind an inline confirmation (`last_way_in` explained in
place), and "Add a passkey" (`N` then `P`), naming the new passkey after the
device. With no passkeys the card SHALL show a quiet panel, "No passkeys yet",
holding the Add button. A refusal for a stale sign-in SHALL offer "Sign in
again" through `/bff/login?max_age=600`. A browser without passkeys SHALL be
told so instead of offered Add. Every string SHALL be in en, pt-PT, es-ES
and fr, and the section usable at 375px.

#### Scenario: Add needs a fresh sign-in
- **WHEN** the person presses Add a passkey an hour after signing in
- **THEN** the section says to sign in again first and offers it, coming back to the page
