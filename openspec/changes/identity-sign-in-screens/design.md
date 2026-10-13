# Design

## Context

S1a (`openspec/changes/archive/2026-10-13-campbooks-identity-s1a/`, ADR
0014) defines the contract these screens sit on:

| Route | What the screen does with it |
| --- | --- |
| `GET /oauth2/authorize` (via `/bff/login`) | with no issuer session, 303 to `/sign-in?request=<id>` |
| `GET /auth/requests/{id}` | `RequestInfo { client_id, providers }`: the ways in, in display order; 404 when the request expired |
| `GET /auth/start/{provider}?request=` | full navigation to the provider |
| `GET /auth/callback/{provider}` | 303 to the client (signed in), to `/sign-in/new?request=`, or to `/sign-in?error=<code>[&request=]` with `invalid_state`, `cancelled_at_provider`, `provider_unavailable`, `login_failed`, `account_disabled` |
| `GET /auth/new-account` | `NewAccount { provider, email, display_name }`; 404 when nothing is pending for this browser |
| `POST /auth/new-account` (`X-Campbooks-CSRF: 1`) | 201 `{ redirect }`; 403 `csrf_check_failed`; 404; 409 `sign_up_expired` / `way_in_belongs_to_another_account` |
| `identity.list_held_grants` (`identity:read`) | the caller's held mailboxes: `{ id, provider, address, display_name, expires_at }` |
| `identity.adopt_held_grant` (`identity:write`) | connect one to a workspace; 404 gone; 409 `account_connected_by_another_member`, `mailbox_unsupported` |
| `identity.discard_held_grant` (`identity:write`) | forget one |

The `/auth/*` routes are on the app's own origin, outside the BFF: the
browser calls them with its issuer cookies (same-origin `fetch`), never with
a token. The API operations go through `/bff/api/*` as usual; the BFF asks
for every API scope, so `identity:read`/`identity:write` are in its token.

**Data ownership.** None new. Accounts, pending sign-ups and held grants live
in schema `identity` (S1a); connections in schema `connectors`. The client
keeps one value per browser tab in `sessionStorage`: the path to return to
after signing in (needed because the BFF keeps `return_to` in its own
cookie, out of the sign-in screen's sight). No events are published or
consumed by the client. No AI use.

**Scopes.** `identity:read` (list held grants), `identity:write` (adopt,
discard), `connectors:read` (list connections). The `/auth/*` screen
routes have no scope (public, cookie- and CSRF-bound, as S1a defines).

**Rails app.** Nothing here is copied from the old app on `main` (email and
password registration with an emailed code, with provider sign-in beside
it). These screens follow S1a's contract: there is no password, and the
provider round trip is the account.

## Decisions

### 1. Landing and sign-in: one screen

The #451 landing showed the brand, one line and a single "Sign in" that went
to `/bff/login`. With S1a, `/bff/login` now arrives at our own `/sign-in`,
which has to show the brand anyway.

- **A. Fold the landing into `/sign-in` (chosen).** A signed-out browser that
  opens any app page goes (client-side, replacing history) to
  `/sign-in?return_to=<that page>`. With no `request` there, the screen
  remembers `return_to` for this tab and navigates to
  `/bff/login?return_to=…`; the issuer comes back to
  `/sign-in?request=<id>`, which shows the brand, the product line and the
  ways in. One screen, one choice (the provider) instead of two clicks. The
  #451 objection to sending people straight to the identity provider ("no
  chance to see what Campbooks is") is gone: the first thing they see is a
  Campbooks screen with the product line. Invitation copy is kept: the
  remembered `return_to` says the person came from an invitation link.
- **B. Keep the landing; its "Sign in" leads to `/sign-in`.** No bounce on
  load and no `sessionStorage`. Costs a click on every sign-in, and two
  pages that say the same thing.
- **C. Show the providers on the landing itself.** Needs the provider list
  without an authorization request, which S1a does not expose (the list
  comes with a request), and every button would still have to bounce
  through `/bff/login` to create one. Rejected: a contract change for no
  user-visible gain over A.

A session that **ends while the app is open** does not bounce: leaving a
page someone is working on would lose what they were doing. The gate keeps
the in-place "You were signed out" page, whose "Sign in again" goes through
`/sign-in?return_to=<this page>`.

`/sign-in` and `/sign-in/new` are **public routes**: the router has a
pathless "gated" layout that renders the session gate, and these two sit
beside it under the root. A browser already signed in that opens `/sign-in`
without a request goes to `return_to` (or `/`).

### 2. Which ways in are shown

The owner asked for Google, Microsoft, Zoho and passkey, with only Google
working in S1, and preferred the server's provider list to "coming soon"
buttons when the contract has one. It does: `RequestInfo.providers`.

- **A. Render exactly what the server lists (chosen).** Known providers
  (`google`, `microsoft`, `zoho`, and `passkey` once S3 lists it) get their
  button, label and copy; unknown strings are ignored. Keys are positional
  (`1`, `2`, `3` in the server's order; passkey is `P`). A self-hoster who
  configured only Zoho sees only Zoho. No dead buttons, nothing to change
  in the client when S2/S3 turn providers on.
- **B. Static list, unconfigured ones disabled with "coming soon".** Shows
  where the product is going, but "coming soon" is not true on a self-hosted
  instance that simply didn't configure Microsoft, and it trains people to
  read past disabled buttons.

When the list is empty (no provider configured) the screen says so and
points to the instance's administrator.

### 3. Explaining the one consent before leaving

Signing up with Google asks, in Google's own consent, for the Gmail scope
(`gmail.modify`) and offline access, and Google shows its "unverified app"
screen until verification (#53). The sign-in screen says, under the Google
button and before anyone leaves: Google will also ask for Gmail, so the
first mailbox is ready; what Campbooks keeps (name, address, and the
permission Google gives, not the Google password); that nothing reaches a
workspace until the person says so, and an unused permission is deleted
after seven days; that the mail permission can be unticked and sign-in still
works; and that Google may warn that it hasn't verified the app, why, and
how to continue. The detail sits behind a "What Campbooks reads and keeps"
disclosure so the screen stays one question; the one-line summary is always
visible.

### 4. The new-account screen

`NewAccount` carries `provider`, `email` and `display_name`; `POST` takes no
body. So "name confirmation" means **showing** the name and address that
will be used, and what happens next (the account is created and signed in,
then a workspace, then the mailbox). The name is not editable here: the
contract has no field for it, and account settings are S2. Create is the
form's submit (`Enter`); "Sign in another way" (`Esc`) returns to
`/sign-in?request=<id>`. The page never suggests that another account uses
the address (S1a's rule: say the same thing whatever the address).

### 5. Where the mailbox step goes

- **A. A step after the workspace is created, in place of "is ready"
  (chosen).** `workspaces`' first-run flow gets an optional `AfterCreate`
  slot. The shell fills it with `identity`'s step: when a grant is held it
  shows "Your Gmail ana@example.com will be connected to {workspace}" with
  Connect / Not now (both go on to Home); otherwise it renders the usual
  "is ready" view. The person sees the workspace's name and colour, so it is
  clear where the mail goes, and the step can't be confused with the
  sign-in.
- **B. A switch on the kind step, before creating (S1a design's sketch).**
  One fewer view, but it asks about mail before the workspace exists, mixes
  two decisions on one page (the design system's "one question per view"),
  and makes Not now look like part of creating the workspace.
- **C. Only on Home.** The least flow, but a new person would land on Home
  without being asked, and the offer is easy to miss. Kept as the
  fallback (see 6), not the first ask.
- **D. Before naming the workspace.** It would have to promise a workspace
  that doesn't exist yet, and people joining by invitation would see it
  before they know where they are.

`workspaces` does not import `identity`: the slot is a component the shell
passes in when building the routes (`app/modules.ts`), the same way module
routes are composed.

### 6. The offer on Home

While a grant is held, Home's mail section shows it: the address, until when
it waits, **Connect** (`identity.adopt_held_grant` for this workspace) and
"Don't connect it" (`identity.discard_held_grant`, after an inline
confirmation, because it can't be undone). After connecting, the section
names the connected mailbox (from `connectors.list_connections`). With no
held grant and no connection it is the #451 "coming soon" placeholder
(connecting another mailbox is S2). Adoption refusals are explained in
place: the mailbox is already connected here by another member, the
mailbox is not supported, or the grant expired.

### 7. Shortcuts

The sign-in keys are registry shortcuts (`sign_in.choice1..3`,
`sign_in.passkey`), so hints, `aria-keyshortcuts` and the `?` sheet stay in
step. `1`/`2` are already `item.choice1/2` (scope `item`), so they would
conflict. A new scope, **`focus`**, is for one-question pages: it overlaps
only `global` and itself, because a focus page never shows a list, an item
or a composer. The alternative, reusing `item.choice1/2`, would label the
keys "Choice 1" in the sheet and has no `3` or `P`. `Esc` uses the global
`overlay.close` binding (latest handler wins), and `Enter` submits the
page's form, shown as a one-off hint on the primary button.

## Error states

| Where | Code | Says | Offers |
| --- | --- | --- | --- |
| `/sign-in?error=` | `cancelled_at_provider` | You cancelled on Google's screen; nothing was created | the ways in again |
| | `invalid_state` | The sign-in took too long or was opened in another browser | Start again |
| | `provider_unavailable` | Google didn't answer; nothing changed | Try again |
| | `login_failed` | Google couldn't confirm who you are | Try again / another way |
| | `account_disabled` | This account is turned off; ask the administrator | — |
| | `way_in_belongs_to_another_account` | That Google account signs in to another Campbooks account | sign in with it |
| | unknown | Something went wrong; nothing changed | Start again |
| `/sign-in?request=` | 404 | This sign-in page expired | Start again |
| `/sign-in/new` | 404 / `sign_up_expired` | The sign-up isn't waiting any more | Sign in again |
| | `way_in_belongs_to_another_account` | as above | Sign in with it |
| | `csrf_check_failed`, network | Couldn't create it; nothing was created | Try again |
| mailbox step / Home | 404 | That mailbox isn't waiting any more | continue / connect later |
| | `account_connected_by_another_member` | Someone else in this workspace already connected it | continue |
| | `mailbox_unsupported` | Campbooks can't read this kind of mailbox yet | continue |

"Start again" goes to `/bff/login` with the remembered `return_to`.

## Risks and trade-offs

- **One redirect before the first paint for a signed-out deep link.** Two
  server redirects (`/bff/login`, `/oauth2/authorize`); both are local and
  fast. If the BFF is not configured, the browser shows the BFF's problem
  page, exactly as the old "Sign in" button did.
- **`sessionStorage` for `return_to`.** Per tab and lost if the person
  switches tabs mid-sign-in; then they land on `/` after signing in, which
  is the BFF's own default. Nothing secret is stored.
- **Positional keys** change meaning if the server reorders providers; the
  hint is always on the button, so what you see is what you press.
