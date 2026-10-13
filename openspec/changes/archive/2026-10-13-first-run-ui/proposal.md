# Proposal

## Why

Campbooks Next has a design system, a web BFF that signs browsers in, and a
`workspaces` API, but the client still opens on a system status page. Nobody
can sign in, create a workspace, switch between workspaces or invite a
colleague from the app. Every domain screen that lands next (mail, schedule,
tasks) needs that frame first: a session, a current workspace in the URL, its
accent, and a shell with navigation and an account menu.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/61.

## What Changes

- **Signed out:** a calm, branded landing (the visor, one line, "Sign in")
  that sends the browser to `/bff/login?return_to=<this page>`. When a
  session ends while the app is open, the landing says so. Signing out
  (account menu) posts to `/bff/logout` and follows the identity provider's
  end-session URL.
- **First run:** a signed-in user without a workspace creates one, one
  question per view: its name, then what it is for (Business or Personal,
  which picks the accent preset), then done. Name and kind can be skipped
  (sensible defaults); only having a workspace is required. The signup
  policy's refusals (`signup_invite_only`, `signup_requires_approval`,
  `signup_code_invalid`) each get clear copy, and a code is asked for only
  when the instance requires one.
- **Shell on real data:** routes `/w/:workspaceId/…`; a sidebar with the
  workspace switcher (⌘⇧W, lists `workspaces.list`, each workspace's accent,
  last workspace remembered on the device), Home and Settings; an account menu
  (name, email, theme, density, language, sign out); a phone header and tab
  bar below 48rem.
- **Empty Home:** "Nothing needs you yet", an honest "Connect your mail"
  call to action that says mail connections are coming (connectors, #36),
  and the keyboard shortcuts that work today.
- **Members and invitations (Settings → Members):** members with their
  roles; owners and admins change roles (with undo) and remove members
  (confirmed inline), invite by email and see pending invitations. Nothing
  emails invitations yet, so the inviter gets the accept link once, right
  after inviting, with a Copy button. An accept-invitation page (`/invitations/accept?token=…`) signs in first when
  needed and then accepts.
- Loading, empty and error states on every screen, shortcuts for the new
  actions in the registry (`workspaces.create`, `workspaces.invite_member`,
  `nav.settings`), layouts down to 375px.
- New design-system components, each with a story and a test: TextField,
  Select, ChoiceCards, FocusPage, Popover, WorkspaceMenu, ListRow,
  InlineConfirm, PhoneHeader, AccountButton, and icons for settings, people,
  plus and sign out.
- The `system` status page moves from `/` to `/system`.

## Capabilities

### New Capabilities

- `first-run-ui`: the web client's session handling, first-run workspace
  creation, workspace shell and switcher, empty Home, and members and
  invitations screens; the invitation link returned once to the inviter.

### Modified Capabilities

None.

## Impact

- Server: `workspaces.invite_member` answers with `accept_url` (the accept
  link, built from the public URL), once; development servers stop logging
  the link. No new endpoint or scope.
- Modules: `clients/app/src/modules/workspaces` is the client
  half of `crates/modules/workspaces` (new); the app shell (`src/app`) owns
  the session gate, landing, workspace layout, account menu and Home. No
  cross-module dependency, no `architecture.toml` entry.
- `lib/api` gains `signOut()`, a problem-code helper and a typed schema
  alias; `lib/ui` gains the components above.
- No new dependencies.

## Non-goals

- Workspace settings beyond members (rename, accent picker, delete, leave)
  and the security log.
- Revoking an invitation (the API has no endpoint for it yet).
- Emailing invitations (the `notifications` module).
- Connecting mail (connectors, #36): the call to action is a placeholder.
- Native (Tauri) sign-in: the native shell owns its session (ADR 0004); the
  session gate is a web concern and is skipped there.
- Inbox, Schedule, Tasks, Documents and Money navigation: those places
  appear when their modules land.
