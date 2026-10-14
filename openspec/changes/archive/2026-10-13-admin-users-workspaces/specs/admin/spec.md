## ADDED Requirements

### Requirement: People and workspaces metadata for administrators
The `workspaces` module SHALL serve, to administrators only, a list of
users (`GET /admin/v1/workspaces/users`, `users.read`) with each user's id,
identity account id, display name, email, first sign-in and number of live
workspaces, and a user's detail (`GET /admin/v1/workspaces/users/{user_id}`)
with every workspace they belong to and their role in it; and a list of
workspaces (`GET /admin/v1/workspaces/workspaces`, `workspaces.read`) with
name, kind, creation and deletion times, member count and owner ids, and a
workspace's detail (`GET /admin/v1/workspaces/workspaces/{workspace_id}`)
with its members and roles and the number of pending invitations. Lists
SHALL be newest first, filtered by an optional case-insensitive prefix of
the name or address (`q`, wildcards taken literally, at most 100
characters), and paged by an opaque cursor (`after`) and `limit` (1 to 200,
default 50). Responses SHALL hold no content and no invitee address.

#### Scenario: Paging through people
- **WHEN** a super admin lists users with `limit=2` on an instance with three users
- **THEN** the two newest come back with a `next` cursor, and asking with `after=<next>` returns the oldest and no cursor

#### Scenario: Search is a literal prefix
- **WHEN** a super admin searches users with `q=%`
- **THEN** no user matches, although every address contains characters

#### Scenario: Pending invitations are counted, not shown
- **WHEN** a super admin opens a workspace with one pending invitation to `dan@example.com`
- **THEN** the detail says `pending_invitations: 1` and `dan@example.com` appears nowhere in the response

#### Scenario: Not an administrator, even with bad parameters
- **WHEN** a member without a profile calls `GET /admin/v1/workspaces/users?limit=0`
- **THEN** the answer is 404, not 400

### Requirement: Account details for administrators
The `identity` module SHALL serve `GET /admin/v1/identity/accounts/{account_id}`
(`users.read`) with the account's display name, primary address and whether
it is verified, creation time, suspension time, provider ways in (provider,
address at link, how and when linked, last use), number of passkeys, number
of live issuer sessions and the time of the last sign-in. It SHALL never
return a session, a token, a passkey's key or a provider subject. An unknown
account, including every account while another identity provider serves the
instance, SHALL answer 404.

#### Scenario: How a person gets in
- **WHEN** a super admin opens the account of a person with a Google way in, a passkey, one live and one ended session
- **THEN** the response lists the Google way in with its address, `passkeys: 1`, `sessions.active: 1` and a last sign-in time, and no session hash or key

### Requirement: Suspending an account
The `identity` module SHALL serve `POST /admin/v1/identity/accounts/{account_id}/suspend`
and `…/unsuspend` (`users.suspend`). Suspending SHALL, in one transaction,
mark the account suspended, end its issuer sessions, revoke its refresh-token
families, record `account_suspended` (with the administrator's account id)
in the account's audit trail and publish `identity.account_suspended`; the
web BFF SHALL then delete the account's web sessions. Both operations SHALL
be idempotent: repeating one changes and records nothing more and answers
the account as it is. Suspending an account that has an admin profile SHALL
be refused with 409 `account_is_admin`, read through the kernel's
`AdminProfiles` port, and change nothing.

#### Scenario: Suspend once, retry safely
- **WHEN** a super admin suspends an account twice
- **THEN** both answers are 200 with the same `suspended_at`, the account has no live session or refresh family, and one audit row and one event exist

#### Scenario: An administrator cannot be suspended over HTTP
- **WHEN** a super admin suspends an account that has an admin profile, asking in Portuguese
- **THEN** the answer is 409 `account_is_admin` whose detail tells, in Portuguese, to revoke the profile with the server's command line first, and the account is not suspended

#### Scenario: Web sessions end at once
- **WHEN** `identity.account_suspended` is delivered for an account signed in on two browsers
- **THEN** both browsers' web sessions are gone and another account's are not

### Requirement: Connection health for administrators
The `connectors` module SHALL serve
`GET /admin/v1/connectors/workspaces/{workspace_id}/connections`
(`workspaces.read`) with each connection of that workspace: provider,
address, the member who connected it, capabilities, health with its reason
and since when, and each stream's capability, phase, paused flag, last sync
and consecutive failures. It SHALL never return credentials, scopes, cursors
or anything read from the mailbox.

#### Scenario: A broken mailbox shows up
- **WHEN** a workspace has a connection whose grant was revoked
- **THEN** its entry says `health: needs_reauth`, `health_reason: grant_revoked`, and its stream is paused with its failure count

### Requirement: People and Workspaces panels
The admin section SHALL offer **People** (a searchable list that shows more
on request; a person's page with their account, ways in, sessions,
workspaces and a Suspend or Unsuspend action) and **Workspaces** (a
searchable list; a workspace's page with its members and its connections'
health), in all four locales, at 375 px and desktop widths, with loading,
empty and error states. Suspend SHALL ask for confirmation in place, saying
that the person is signed out everywhere and cannot sign in until
unsuspended, and that nothing is deleted.

#### Scenario: Suspending from the People page
- **WHEN** a super admin opens a person, chooses Suspend and confirms
- **THEN** the account shows as suspended with the time, and the action becomes Unsuspend

#### Scenario: An administrator's account
- **WHEN** the confirmation is answered `account_is_admin`
- **THEN** the page explains that the admin profile must be revoked with the server's command line first, and nothing changes
