## Purpose

How the web client gets a person from signed out to a workspace: the session
gate and sign-out, creating the first workspace, the workspace shell and
switcher, the empty Home, and members and invitations.

## ADDED Requirements

### Requirement: Signed-out landing
When `GET /bff/session` reports no session, the web client SHALL show a
landing page at the requested URL instead of that page, with one "Sign in"
action that navigates to `/bff/login` with `return_to` set to the requested
path, query and fragment. When a session that was live ends while the app is
open, the landing SHALL say that the session ended. When the session cannot
be read, the client SHALL say that Campbooks can't be reached and offer to
try again. Inside the native shell the gate SHALL NOT apply.

#### Scenario: Deep link while signed out
- **WHEN** a signed-out browser opens `/w/1234/settings/members`
- **THEN** the landing is shown and its "Sign in" link is `/bff/login?return_to=%2Fw%2F1234%2Fsettings%2Fmembers`

#### Scenario: Session ended
- **WHEN** the session query reports `authenticated: false` after it had reported a signed-in user
- **THEN** the landing shows that the session ended

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
approval and offer to check again.

#### Scenario: Create with defaults
- **WHEN** a user skips the name and the kind
- **THEN** `workspaces.create` is called with the localized default name and kind `business`, and the done view offers to open the workspace

#### Scenario: Instance requires a code
- **WHEN** `workspaces.create` answers 403 `signup_code_invalid`
- **THEN** the flow shows a view asking for the signup code, and submitting it calls `workspaces.create` again with `signup_code`

### Requirement: Workspace shell
Inside a workspace the client SHALL show a sidebar (from 48rem) or a phone
header and tab bar (below 48rem) with the workspace switcher, Home and
Settings, and an account menu with the user's name and email, theme (system,
light, dark), density, language and Sign out. The switcher SHALL open with
⌘⇧W (Ctrl+Shift+W elsewhere), list the user's workspaces with their accent,
mark the current one, and offer to create a workspace. Opening a workspace
SHALL apply its accent and remember it as the last workspace on this device.

#### Scenario: Switch workspace from the keyboard
- **WHEN** the user presses ⌘⇧W and picks another workspace
- **THEN** the client navigates to that workspace and the document's accent becomes that workspace's accent

### Requirement: Empty Home
A workspace's Home SHALL say that nothing needs the user yet, offer
"Connect your mail" as unavailable with copy saying that mail connections are
coming, and show the keyboard shortcuts available on the page.

#### Scenario: New workspace
- **WHEN** a user opens a workspace's Home
- **THEN** "Nothing needs you yet" is shown and the "Connect your mail" button is disabled with a note that it is coming

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
