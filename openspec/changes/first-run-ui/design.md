# Design

## Context

The web client talks to the API only through the server's BFF
(`openspec/changes/web-bff/`, ADR 0004): `GET /bff/session` says who is
signed in, `/bff/login?return_to=` starts a sign-in, `POST /bff/logout` ends
it and returns the identity provider's end-session URL, and `/bff/api/*`
answers `401 session_required` when there is no session (the web transport
then sends the browser to sign in and back). The `workspaces` module
(`openspec/changes/workspaces-tenancy/`) provides the operations this change
calls: `workspaces.list`, `.create`, `.get`, `.list_members`,
`.change_role`, `.remove_member`, `.list_invitations`, `.invite_member` and
`.accept_invitation`.

Data ownership: none new. Everything is read and written through existing
`workspaces` operations, whose data lives in Postgres schema `workspaces`; the
BFF session lives in schema `bff`. The client stores only UI preferences on
the device (`lib/storage`): theme, density, language, the accent per
workspace and the last workspace opened. No events are published or
consumed. Scopes are the existing ones: `workspaces:read` (list, get,
members), `workspaces:write` (create, accept) and `workspaces:admin`
(invitations, roles, removal). There is no AI use.

The UI is built from `lib/ui` only; components that did not exist are added
there, each with a story and a test (AGENTS.md → UI).

## Decisions

### The session gate wraps every route

The root route renders a gate around the outlet. While `/bff/session` loads
it shows the visor; when it fails it shows "Can't reach Campbooks" with Try
again; when the browser is signed out it renders the landing **in place, at
the URL the user asked for**, so "Sign in" carries that URL as `return_to`
and the user comes back exactly there (an invitation link included). When a
session that was live ends (the session query refetches on focus), the
landing says the session ended. Inside Tauri the gate is skipped: the native
shell owns sign-in.

Alternatives considered:

- *A `/sign-in` route and redirects from a `beforeLoad` guard.* Adds a URL
  that means nothing to the server, needs the original URL threaded through
  a search parameter, and briefly mounts the target route before
  redirecting. No gain over rendering in place.
- *Let the server refuse to serve the SPA without a session and redirect to
  `/bff/login` itself.* Removes the landing (the user lands straight on the
  identity provider, with no chance to see what Campbooks is) and couples the
  static file server to the BFF. Rejected.

### The current workspace lives in the URL

Routes are `/w/:workspaceId/…`. The id is the only source of truth for the
open workspace: links can be shared and opened in several tabs, and nothing
can disagree with the address bar. `/` resolves the target: the last
workspace remembered on this device if the user still belongs to it, else
the first one, else the first-run flow. The layout route reads the
workspace from `workspaces.list` (already cached for the switcher) and shows
"This workspace isn't available" when it isn't in the list, which is what a
404 from the API means (permission failures on reads are 404s).

Alternative considered: *a selected workspace kept in client state*, with
URLs free of ids. Simpler URLs, but a reload, a second tab or a pasted link
would open the wrong workspace, and every query would have to read hidden
state. Rejected.

The accent follows the workspace: `ThemeProvider` moves inside the root route
so its `workspaceId` is the route's (or the last workspace outside one), and
the layout applies the server's accent for the workspace, which the device
remembers so the boot script paints it before the next load. The first-run
kind step previews the chosen accent on the page only (a `data-accent`
attribute on its container), without touching the device's settings.

### First run: one question per view, skippable except the workspace

Steps are local state in one route (`/welcome`): name → kind → done. Each
view asks one thing, with Back where there is a previous step and Skip where
a default is sensible (the name defaults to "My workspace", the kind to
Business). The workspace itself cannot be skipped: nothing else in the app
works without one, and the copy says so. A signup-policy refusal replaces
the steps with its own view: `signup_code_invalid` asks for the code (a
fourth view that only appears on instances that need it) and retries;
`signup_invite_only` explains that someone has to invite you;
`signup_requires_approval` says the request is waiting and offers Check
again. The same flow, with Cancel, creates further workspaces from the
switcher.

Alternative considered: *one form with name, kind and code together.*
Shorter, but asks for a code most users never need and breaks the
one-question-per-view rule set for setup flows. This flow follows that rule;
it copies no code or pattern from the Rails app on `main`, whose
server-rendered registration (name and email, a one-time code, a password)
is the identity provider's job here.

### Members: inline confirmation, undo where the API can undo

Changing a role applies at once and offers Undo in the toast (Z), which
calls `workspaces.change_role` back to the previous role. Removing a member
cannot be undone through the API (they would need a new invitation), so it
asks first, inline in the row ("Remove Ana? They lose access right away."),
never in a modal. Inviting is a submitted form; inviting the same address
again replaces the earlier invitation, which the list offers as "Send
again" for expired ones. The page shows the controls only to owners and
admins (the caller's role comes with the workspace); an admin cannot change
or remove an owner, and the API's `last_owner` refusal is explained in the
row. Who "you" are in the list is matched by email, the only identifier the
session and the member list share.

### Handing on the invitation link

Nothing emails invitations yet, and a development-only log line left every
other instance with no way to deliver one. `workspaces.invite_member` now
answers with `accept_url`, the accept link built from the instance's public
URL (`CAMPBOOKS_PUBLIC_URL`), in that one response only: the token is still
stored only as a hash, never listed (`list_invitations` returns no link or
token), never logged (the development log line is gone) and never in an
event. Inviting the same address again replaces the invitation and returns
a fresh link. The members page shows the link right after inviting (and
after "Invite again") with a Copy button (C) and says plainly that
Campbooks does not email invitations yet; the cached invitation list keeps
the invitation without the link.

Alternatives considered: *keep logging the link* (works only for whoever
can read server logs, and puts a credential in logs); *an endpoint that
reveals a pending invitation's link again* (needs the token stored
recoverably, a weaker secret store than a hash). Returning it once, like an
API key, is the smallest honest step until notifications send the email.

### Accepting an invitation

`/invitations/accept?token=…` sits behind the session gate like every route,
so a signed-out user sees the landing with invitation copy and signs in back
to the same URL. Signed in, the page accepts once on arrival (guarded
against a double run) and then offers to open the workspace, remembering it
as the last one. Each refusal (`invitation_used`, `invitation_expired`,
`already_a_member`, `invitation_email_mismatch`, not found, no token) has
its own copy and next step.

### Shortcuts for the new actions

`nav.settings` (G then ,) is a client command in the default catalogue.
Operations are keyed by their operationId (ADR 0013) and defined by the
`workspaces` client module (`workspaces.create`: N then W;
`workspaces.invite_member`: I, on the members page). The module manifest
hands module shortcuts to the app's registry when it is created, the same
way it hands over routes, so the conflict check covers them at startup.

Alternative considered: *each screen calls `registry.define` when it mounts.*
Definitions would appear and disappear with screens, `?` would not list them
until visited, and a second mount would throw on the duplicate id.
Rejected.

### Branching on problem codes

Every refusal the UI explains is matched on the problem's stable `code`
(`last_owner`, `signup_code_invalid`, ...), never on its `title` or `detail`,
which the server localizes. `lib/api` exposes it as `problemCode(error)`, and
the web transport detects an ended session by `code = session_required`. The
transport also sends the UI's language as `Accept-Language`, so any problem
text the server writes matches the language the user picked (it can differ
from the browser's). Saving that choice to the user's profile is a follow-up
(#69); until then the account menu stores it on the device.

### Testing and recordings without a backend

Component and route tests stub `fetch` per method and path (the existing
`test/render.tsx` helper, extended), so they run the real transport and
query code. Stories for the new screens stub `fetch` the same way in a
`beforeEach`. The PR recording drives the real app on the Vite dev server
with Playwright intercepting `/bff/*` and answering from in-memory demo data
(`recordings/scenarios/first-run.ts`), so no server, identity provider or
Docker is needed.

Alternatives considered:

- *Mock Service Worker.* A new dependency, a generated worker script in
  `public/` that would ship with the web build unless excluded, and a
  dev-only switch in the app's startup. The fetch stub and Playwright
  interception give the same coverage with no new code in the shipped app.
- *A fake BFF mode in `campbooks-server`.* Server code that exists only for
  demos, and still needs Postgres to run. Rejected.

## Risks / Trade-offs

- Matching "you" by email fails when a provider gives no email; the row then
  shows no "You" label and the server still refuses illegal actions
  (`last_owner`). A future `user_id` in the session would remove the guess.
- The last workspace is remembered per device only; a new device opens the
  first workspace.
- No revoke for invitations yet; expired invitations stay listed until the
  API offers one.
- A lost link cannot be shown again; inviting again issues a new one (and
  invalidates the old).

## Migration Plan

No data to migrate. API clients see one new response field. The system status page moves from `/` to
`/system`.
