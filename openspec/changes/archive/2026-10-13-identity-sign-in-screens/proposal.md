# Proposal

## Why

ADR 0014 makes the sign-in, new-account and "connect this mailbox here"
screens Campbooks screens. Slice S1a (campbooks-cloud#77) built the server
side: the in-process issuer, Google sign-in that asks for the Gmail grant in
the same consent, the new-account step and the held mailbox grant. Until
now its redirects to `/sign-in` and `/sign-in/new` reached the web app's
"page not found", and nobody could adopt a held grant from the UI.

This change is **slice S1b** (campbooks-cloud#78): the screens on top of
S1a's contract, in `clients/app`.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/78
(parent: https://github.com/notacamp/campbooks-cloud/issues/75).

**Modules touched** (client only, `clients/app`):
- new client module **`identity`** (mirrors `crates/modules/identity`):
  `/sign-in`, `/sign-in/new`, the first-run mailbox step and the held
  mailbox offer on Home;
- **`workspaces`**: the first-run flow takes an optional step after the
  workspace is created (a slot the shell fills, so `workspaces` does not
  import `identity`);
- **`connectors`**: a `connectors.list_connections` query, so Home can say
  which mailbox is connected;
- **app shell**: public routes outside the session gate; a signed-out
  browser goes to the sign-in screen;
- **`lib/api`**: a typed client for the server's own web routes (`/bff/*`,
  `/auth/*`);
- **`lib/shortcuts`**: a `focus` scope for one-question pages.

No server code, endpoint or schema changes; no new dependency.

## What Changes

- **`/sign-in`**: the brand and one line about Campbooks, then one button per
  way in the server lists for the waiting request (`GET /auth/requests/{id}`),
  numbered `1`, `2`, `3` in its order, plus "Use a passkey" (`P`) when the
  server lists it. Before anyone leaves for Google, it says what the one
  consent covers (Gmail is connected too, what is read and kept, that the
  mail permission can be unticked) and that Google may show an
  "unverified app" warning, and why. Errors from S1a's callback
  (`?error=`) are explained in place, each with what to do.
- **`/sign-in?return_to=`**: with no waiting request, the screen starts one
  through `/bff/login?return_to=…`, so signing in comes back to the page that
  was opened. A signed-out browser opening any app page goes here.
- **`/sign-in/new`**: the name and address Google gave, what happens next,
  Create (`Enter`) or "Sign in another way" (`Esc`). `sign_up_expired`,
  `way_in_belongs_to_another_account`, `csrf_check_failed` and a sign-up that
  is no longer waiting each get their own explanation.
- **First-run mailbox step**: after the workspace is created, when a mailbox
  grant is held, "Your Gmail ana@example.com will be connected to this
  workspace" with Connect (`Enter`) or Not now (`Esc`). Not now is never a
  dead end: Home keeps offering it until the grant expires (seven days).
- **Home**: the held mailbox offer (Connect, or forget it); a connected
  mailbox is named; otherwise "Connect your mail" stays "coming soon".
- **Signed out while using the app**: the in-place "You were signed out"
  page stays, its "Sign in again" going through `/sign-in`.
- Stories for every screen state under "Screens/Sign in"; recording
  scenario `sign-in`.

## Non-goals

- Server changes. The screens use S1a's contract as it is. Where the
  contract lacks something (an editable name on the new-account screen,
  passkeys, Microsoft and Zoho), the screen shows only what exists (see
  design).
- Microsoft, Zoho and passkey **flows** (S2, S3). Their buttons, keys and
  copy are built and appear as soon as the server lists them.
- Settings → Ways to sign in, linking another mailbox, reconnecting (S2).
- The consent screen for third-party clients (S4).
- Provider brand marks in colour: the design system has no colours for
  them (Ink accent, no blue); buttons carry the provider's name.
