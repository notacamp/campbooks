# Tasks

## 1. Foundations

- [ ] 1.1 `lib/api`: typed client for the server's web routes (`/bff/*`, `/auth/*`), same-origin with the CSRF header; unit test
- [ ] 1.2 `lib/shortcuts`: `focus` scope (overlaps only `global` and itself); registry test
- [ ] 1.3 Router: pathless gated layout; public routes beside it; the gate sends a signed-out page load to `/sign-in?return_to=`

## 2. `identity` client module

- [ ] 2.1 Queries: sign-in request, pending account, create account, held grants, adopt, discard
- [ ] 2.2 `/sign-in`: ways in from the server's list with positional keys, passkey, consent explanation and unverified-app note, every callback error, expired request, no provider, bounce through `/bff/login` with a remembered `return_to`
- [ ] 2.3 `/sign-in/new`: name and address, what happens next, Create (Enter), Sign in another way (Esc), every failure
- [ ] 2.4 Mailbox step after the workspace is created (via the `workspaces` `AfterCreate` slot) and the held offer on Home
- [ ] 2.5 i18n namespace `identity` in en, pt-PT, es-ES and fr

## 3. Shell, `workspaces` and `connectors`

- [ ] 3.1 `workspaces`: `AfterCreate` slot in the first-run flow
- [ ] 3.2 `connectors`: `connectors.list_connections` query; Home names a connected mailbox
- [ ] 3.3 In-place "You were signed out" page leads through `/sign-in`

## 4. Verification

- [ ] 4.1 Route and component tests with a stubbed API; stories for every screen state under "Screens/Sign in" with axe in light and dark
- [ ] 4.2 Recording scenario `sign-in` (375px + desktop) with intercepted server responses (demo data)
- [ ] 4.3 CHANGELOG updated; client gates green; OpenSpec strict validation; UI verified at 375px and desktop
