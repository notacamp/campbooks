# first-run-ui Specification

## Purpose
How the web client gets a person from signed out to a workspace: the session
gate and sign-out, creating the first workspace, the workspace shell and
switcher, the empty Home, and members and invitations.

## Requirements

### Requirement: Signed-out landing
When `GET /bff/session` reports no session on a page load, the web client
SHALL replace the requested page with `/sign-in?return_to=<path, query and
fragment>`. When a session that was live ends while the app is open, the
client SHALL instead show, at the same URL, that the session ended, with a
"Sign in again" action leading to `/sign-in` with that `return_to`. When
the session cannot be read, the client SHALL say that Campbooks can't be
reached and offer to try again. `/sign-in` and `/sign-in/new` SHALL be shown
without a session. Inside the native shell the gate SHALL NOT apply.

#### Scenario: Deep link while signed out
- **WHEN** a signed-out browser opens `/w/1234/settings/members`
- **THEN** the client goes to `/sign-in?return_to=%2Fw%2F1234%2Fsettings%2Fmembers`

#### Scenario: Session ended
- **WHEN** the session query reports `authenticated: false` after it had reported a signed-in user
- **THEN** the page shows that the session ended, and "Sign in again" leads to `/sign-in` with the current page as `return_to`

### Requirement: Sign out
The account menu SHALL offer "Sign out", which posts to `/bff/logout` with
the CSRF header and then navigates to the returned `logout_url`, or to the
landing when it is `null`.

#### Scenario: Sign out with an identity provider session
- **WHEN** the user chooses "Sign out" and the BFF answers with a `logout_url`
- **THEN** the browser navigates to that URL

### Requirement: Choosing the workspace to open
The client SHALL address the open workspace in the URL (`/w/:workspaceId`).
At `/` it SHALL open the workspace last opened on this device when the user
still belongs to it, else their first workspace, else the first-run flow.
A workspace id that is not among the user's workspaces SHALL show that the
workspace isn't available, with a way to open another one.

#### Scenario: Returning user
- **WHEN** a signed-in user who last opened workspace B (and belongs to A and B) opens `/`
- **THEN** the client navigates to `/w/B`

#### Scenario: No workspace yet
- **WHEN** a signed-in user with no workspace opens `/`
- **THEN** the client navigates to the first-run flow

### Requirement: First workspace
The first-run flow SHALL ask one question per view: the workspace's name,
then whether it is for Business or Personal use (which picks the accent
preset, previewed on the page), then confirm it is ready. The name and kind
views SHALL offer Skip (defaults: a localized "My workspace", Business) and
the kind view SHALL offer Back. Creating the workspace SHALL be the only
step that cannot be skipped. When the API refuses with
`signup_code_invalid`, the flow SHALL ask for the signup code and retry with
it; with `signup_invite_only` it SHALL explain that an invitation is needed;
with `signup_requires_approval` it SHALL explain the request is waiting for
approval and offer to check again. When the person has a held mailbox grant
once the workspace exists, the mailbox step SHALL replace the "ready" view.

#### Scenario: Create with defaults
- **WHEN** a user with no held mailbox skips the name and the kind
- **THEN** `workspaces.create` is called with the localized default name and kind `business`, and the done view offers to open the workspace

#### Scenario: Instance requires a code
- **WHEN** `workspaces.create` answers 403 `signup_code_invalid`
- **THEN** the flow shows a view asking for the signup code, and submitting it calls `workspaces.create` again with `signup_code`

### Requirement: Workspace shell
Inside a workspace the client SHALL show a sidebar (from 48rem) or a phone
header and tab bar (below 48rem) with the workspace switcher, Home, Inbox
with its streams, and Settings, and an account menu with the user's name and
email, theme (system, light, dark), density, language and Sign out. The
switcher SHALL open with ⌘⇧W (Ctrl+Shift+W elsewhere), list the user's
workspaces with their accent, mark the current one, and offer to create a
workspace. Opening a workspace SHALL apply its accent and remember it as the
last workspace on this device.

#### Scenario: Switch workspace from the keyboard
- **WHEN** the user presses ⌘⇧W and picks another workspace
- **THEN** the client navigates to that workspace and the document's accent becomes that workspace's accent

#### Scenario: Go to the inbox
- **WHEN** the user presses G then I
- **THEN** the client opens the workspace's Primary stream

### Requirement: Empty Home
While the user can see no inbox in the workspace, its Home SHALL say that
nothing needs the user yet and show the keyboard shortcuts available on the
page; once they can see one, Home SHALL show what needs them instead
(home-ui). The empty Home's mail section SHALL offer the person's held
mailbox grant when there is one, else name the mailboxes connected to the
workspace that the person can see, else offer "Connect your mail" as
unavailable with copy saying that connecting more mailboxes is coming.

#### Scenario: New workspace
- **WHEN** a user with no held mailbox and no connection opens a workspace's Home
- **THEN** "Nothing needs you yet" is shown and the "Connect your mail" button is disabled with a note that it is coming

#### Scenario: Mailbox connected
- **WHEN** `connectors.list_connections` returns a connection for `ana@example.com` and its inbox is not listed yet
- **THEN** Home says that `ana@example.com` is connected

#### Scenario: The inbox is there
- **WHEN** `conversations.list_inboxes` lists an inbox for the user
- **THEN** Home shows what needs them instead of "Nothing needs you yet"

### Requirement: Members and invitations
Settings → Members SHALL list the workspace's members with their role. For
owners and admins it SHALL allow changing a member's role (applied at once,
with Undo that restores the previous role), removing a member after an
inline confirmation, inviting by email with a role (member or admin), and
listing pending invitations with their expiry. Admins SHALL NOT be offered
actions on owners or the owner role. The API's `last_owner` and
`already_a_member` refusals SHALL be explained in place. Other members SHALL
see the list without actions.

#### Scenario: Change a role and undo
- **WHEN** an owner changes a member's role from member to admin and then presses Undo
- **THEN** `workspaces.change_role` is called with `admin` and then with `member`

#### Scenario: Remove with inline confirmation
- **WHEN** an admin chooses Remove on a member
- **THEN** a confirmation appears in that row, and `workspaces.remove_member` is called only when it is confirmed

### Requirement: Invitation link returned once
`workspaces.invite_member` SHALL answer 201 with the invitation and
`accept_url`, the link `<public URL>/invitations/accept?token=<token>`. The
link SHALL appear in no other response, log line or event, and
`workspaces.list_invitations` SHALL never include a token or link. Inviting
the same address again SHALL return a new link and invalidate the previous
one. The members page SHALL show the link right after inviting, with a Copy
action that has a shortcut, and SHALL say that Campbooks does not email
invitations yet.

#### Scenario: Link returned once
- **WHEN** an owner invites `bea@example.com`
- **THEN** the response carries `accept_url` on the public URL, and listing invitations shows no link or token

#### Scenario: Fresh link on re-invite
- **WHEN** the owner invites `bea@example.com` again
- **THEN** a different `accept_url` is returned, and only its token can be accepted

### Requirement: Accepting an invitation
`/invitations/accept?token=…` SHALL, for a signed-in user, accept the
invitation once and then offer to open the workspace. A signed-out user SHALL
see the landing with invitation copy and come back to the same URL after
signing in. `invitation_used`, `invitation_expired`, `already_a_member`,
`invitation_email_mismatch`, an unknown invitation and a missing token SHALL
each be explained with a next step.

#### Scenario: Accept after signing in
- **WHEN** a signed-in user opens a valid invitation link
- **THEN** `workspaces.accept_invitation` is called once with the token and the page offers to open the joined workspace

#### Scenario: Wrong account
- **WHEN** the API answers 403 `invitation_email_mismatch`
- **THEN** the page says the invitation was sent to another address and offers to sign out

### Requirement: Accessible, localized, keyboard-first screens
Every screen in this capability SHALL exist in en, pt-PT, es-ES and fr, show
loading, empty and error states, fit a 375px-wide viewport without
horizontal scrolling, pass axe in light and dark, and show the shortcut of
every action that has one, from the shortcut registry.

#### Scenario: Shortcut hints
- **WHEN** the members page is open for an admin
- **THEN** the invite action shows the keys of `workspaces.invite_member` from the registry

### Requirement: Sign-in screen
`/sign-in?request=<id>` SHALL read `GET /auth/requests/{id}` and show the
brand, one line about Campbooks and one action per way in the response lists
that the client knows (`google`, `microsoft`, `zoho`, `passkey`), in the
listed order, ignoring unknown ones. Provider actions SHALL navigate to
`/auth/start/<provider>?request=<id>` and SHALL have the keys `1`, `2` and
`3` by position. The passkey action SHALL have `P`, SHALL be shown only in a
browser with passkeys, and SHALL run the passkey ceremony in place
(`/auth/passkey/sign-in/options`, the browser's dialog,
`/auth/passkey/sign-in`), then follow the answer's redirect. Where the
browser offers passkeys in a field's autofill, the screen SHALL also show a
"Saved passkeys" field (`autocomplete="username webauthn"`) whose autofill
ceremony starts when the screen opens. A passkey that is not accepted, a
ceremony that expired or was cancelled, and too many tries SHALL be explained
in place. When the request says passkey sign-up is open, the screen SHALL
link to `/sign-in/passkey?request=<id>`, a one-question screen (what to call
you; Create with Enter, Sign in another way with Escape) that creates the
account with a new passkey and explains a refusal (`signup_invite_only`, a
browser without passkeys) on its own page. With no way in listed,
the screen SHALL say that sign-in is not set up and to ask the instance's
administrator. A request the server no longer knows (404) SHALL be
explained, with an action that starts again. Without `request`, the screen
SHALL remember `return_to` for the tab and navigate to
`/bff/login?return_to=<return_to>` (`/` when absent or not a same-origin
path); a browser already signed in SHALL go to `return_to` instead. When the
remembered `return_to` is an invitation link, the screen SHALL use the
invitation copy.

#### Scenario: Google only
- **WHEN** `/sign-in?request=r1` loads and the request lists `["google"]`
- **THEN** one "Continue with Google" link to `/auth/start/google?request=r1` is shown with the key hint `1`, and pressing `1` follows it

#### Scenario: Every way in
- **WHEN** the request lists `["google", "microsoft", "zoho", "passkey"]`
- **THEN** Google, Microsoft and Zoho have keys `1`, `2` and `3`, and "Use a passkey" has `P`

#### Scenario: Sign in with a passkey from the screen
- **WHEN** a browser with passkeys presses `P` on `/sign-in?request=r1` and approves a passkey
- **THEN** the screen posts the assertion and follows the returned redirect to the web client

#### Scenario: Passkey cancelled
- **WHEN** the person closes the browser's passkey dialog
- **THEN** the screen says no passkey was used and "Use a passkey" can be pressed again

#### Scenario: Start from a deep link
- **WHEN** a signed-out browser opens `/sign-in?return_to=%2Fw%2F1234`
- **THEN** it navigates to `/bff/login?return_to=%2Fw%2F1234`

### Requirement: The one consent explained before leaving
When Google is offered, the sign-in screen SHALL say, before the person
leaves for Google, that Google will also ask for access to their Gmail so
their first mailbox is ready; what Campbooks keeps (their name, address and
the permission Google gives, never the Google password); that the mailbox is
connected to a workspace only when they choose, and an unused permission is
deleted after seven days; that unticking mail access on Google's screen still
signs them in; and that Google may warn that it has not verified the app,
with the reason and how to continue.

#### Scenario: Read before redirecting
- **WHEN** the sign-in screen offers Google
- **THEN** the one-line summary of the Gmail access and the unverified-app note are visible without opening anything, and the details are one disclosure away

### Requirement: Sign-in errors explained
The sign-in screen SHALL explain each `error` the identity module sends it
(`cancelled_at_provider`, `invalid_state`, `provider_unavailable`,
`login_failed`, `account_disabled`, `way_in_belongs_to_another_account`,
`signup_invite_only`)
and any other code generically, each saying what happened and what to do,
above the ways in when the request is still waiting, or with an action that
starts again when it is not.

#### Scenario: Cancelled at Google
- **WHEN** `/sign-in?error=cancelled_at_provider&request=r1` loads
- **THEN** the screen says the sign-in was cancelled on Google's screen and nothing was created, and the ways in are shown again

#### Scenario: State expired
- **WHEN** `/sign-in?error=invalid_state` loads
- **THEN** the screen says the sign-in took too long or was opened in another browser, and "Start again" navigates to `/bff/login` with the remembered `return_to`

### Requirement: New-account screen
`/sign-in/new?request=<id>` SHALL read `GET /auth/new-account` and show the
name and address the provider gave, the provider, and what happens next
(the account is created and signed in, then a first workspace, then the
choice to connect the mailbox). Create SHALL post to `/auth/new-account`
with `X-Campbooks-CSRF: 1` and, on 201, navigate to the returned `redirect`.
"Sign in another way" (`Esc`) SHALL return to `/sign-in?request=<id>`. A 404
or `sign_up_expired` SHALL say the sign-up is no longer waiting and offer to
sign in again; `way_in_belongs_to_another_account` SHALL say that this
provider account signs in to another Campbooks account; `signup_invite_only`
SHALL say that the instance is by invitation only and offer to sign in with
another account; any other failure
SHALL say that nothing was created and offer to try again. The screen SHALL
NOT say whether another account uses the address.

#### Scenario: Create the account
- **WHEN** a person on `/sign-in/new?request=r1` presses Enter
- **THEN** `POST /auth/new-account` is sent with `X-Campbooks-CSRF: 1`, and the browser navigates to the `redirect` it returns

#### Scenario: Sign-up expired
- **WHEN** the post answers 409 `sign_up_expired`
- **THEN** the screen says the sign-up is no longer waiting and offers to sign in again

### Requirement: First-run mailbox step
After the first-run flow creates a workspace, when `identity.list_held_grants`
returns a grant, the client SHALL show "Your Gmail <address> will be
connected to <workspace>" in the workspace's accent, what Campbooks will do
with it, Connect (`Enter`) and Not now (`Esc`). Connect SHALL call
`identity.adopt_held_grant` for that workspace and grant, then open the
workspace's Home. Not now SHALL open Home without changing anything. A
refusal (`account_connected_by_another_member`, `mailbox_unsupported`, or a
grant that is gone) SHALL be explained, with a way on to Home.

#### Scenario: Connect
- **WHEN** a person with a held grant for `ana@example.com` creates "Duarte & Pessoa" and presses Enter on the mailbox step
- **THEN** `identity.adopt_held_grant` is called for the new workspace and that grant, and Home opens

#### Scenario: Not now
- **WHEN** the person chooses Not now
- **THEN** Home opens and offers to connect `ana@example.com` while the grant is held

### Requirement: Held mailbox offer on Home
While the person has a held grant, Home SHALL offer it with the address,
until when it waits, Connect (`identity.adopt_held_grant` for the open
workspace) and "Don't connect it" (`identity.discard_held_grant` after an
inline confirmation). After Connect, Home SHALL name the connected mailbox.

#### Scenario: Forget the mailbox
- **WHEN** the person chooses "Don't connect it" and confirms
- **THEN** `identity.discard_held_grant` is called and the offer is gone
