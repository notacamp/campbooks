# Auth, registration, 2FA & onboarding — `/api/app`

**Status:** ⬜ not started · **Priority:** P0 (blocks every other surface) · **Depends on:** —

The SPA can't render anything until a user can sign in and the API can resolve who they are.
This is the foundation: **session-backed bearer tokens**, the login/registration/2FA flows
that mint them, and the identity resolver every other `/api/app` controller inherits.

## The token model

**Bearer, session-backed.** Reuse the existing `Session` model (`app/models/session.rb`) —
it already carries the inactivity window (`INACTIVITY_LIMIT = 30.days`, `#expired?`,
`#touch_if_stale`), audit logging (`AuditEvent.log("sign_in", …)`), and the prune job. We add
a **bearer** view of it; the cookie path stays for the legacy Hotwire app during coexistence.

**Recommended: `Session#signed_id` as the bearer — no migration.**
- Mint: `session.signed_id(purpose: :api_app)` after `start_new_session_for`-equivalent logic.
- Resolve: `Session.find_signed(token, purpose: :api_app)` → reject if `nil` or `#expired?` →
  `session.touch_if_stale` → set `Current`.
- Tamper-proof (signed), revocable (destroy the `Session` row → token dies), no schema change,
  carries the session id so `user_agent`/`ip_address`/expiry all still apply.
- **Alternative** if you want opaque/short tokens + a server-side revocation list: add
  `sessions.token` (`string`, unique, indexed, `has_secure_token`-style secret) and resolve by
  it instead. Costs a migration; buys shorter tokens. Default to `signed_id` unless the mobile
  team needs the shorter form.

**Identity resolver** — new `Api::App::BaseController`, modeled on
`Api::V1::BaseController#establish_acting_identity!` but Session-based:
```
before_action :authenticate_session_token!   # Bearer → Session → Current.user/workspace, else 401
```
Same **fail-closed** rules as v1: unknown/expired token → 401; deletion-requested user → 401.
Same JSON envelope (`{ data }` / `{ error: { code, message } }`) and **404-not-403** leak rule.
Client stores the token in secure storage (Capacitor) / memory + refresh (web).

## What the SPA must render / do

Sign in (password) · second-factor challenge (TOTP, passkey/WebAuthn, email OTP, recovery
code) · sign out · forgot/reset password · 3-step registration (name/email → 6-digit OTP →
password → workspace) · accept invitation · OAuth sign-in (Google/Zoho/Microsoft) incl. the
native deep-link handoff · onboarding (persona, doc-type/tag suggestions, first-sync status) ·
one-time setup cards + tours dismissal · the `me` bootstrap payload.

## Web routes being replaced

| Web route | Controller#action | Notes |
|---|---|---|
| `POST /session` | `sessions#create` | password login; may branch to MFA |
| `GET /session/{zoho,google,microsoft}` | `sessions#*` | OAuth sign-in kickoff |
| `GET /session/native?token=` | `sessions#native` | redeem one-time `:native_session` token → Session |
| `GET/POST /session/challenge` (+ `passkey_options`, `send_email_code`) | `session_challenges#*` | 2nd-factor step |
| `DELETE /session` | `sessions#destroy` | sign out |
| `*/passwords` | `passwords#*` | reset by token |
| `/registration` (+ `verify`,`check_code`,`resend_code`,`password`,`complete`,`pending_approval`) | `registrations#*` | 3-step signup |
| `GET/POST /invitations/:token(/accept)` | `invitations#*` | invite acceptance |
| `/onboarding` (+ `snooze`,`suggest_document_types`,`suggest_tags`,`first_sync_status`,`apply_persona`,`skip_first_sync`) | `onboarding#*` | onboarding wizard |
| `GET/PATCH /setup/:id`, `POST /setup/dismiss` | `setup#*` | one-time setup cards |
| `POST /tours/:key/dismiss` | `tours#dismiss` | guided-overlay dismissal |
| `/ai_setup/:kind(/message,/apply)` | `ai_setup_chats#*` | conversational doc-type/tag setup |

## `/api/v1` coverage today

❌ **None.** `/api/v1` assumes an already-minted Doorkeeper token; it has no login,
registration, 2FA, password-reset, or `me`-bootstrap. All net-new.

## `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `POST /api/app/session` | Password login → `{ token }` **or** `{ mfa_token, methods: [...] }` | `SessionsController#create` logic, `start_new_session_for` |
| `POST /api/app/session/challenge` | Submit a 2nd factor (`{ mfa_token, method, code/assertion }`) → `{ token }` | `SessionChallengesController`, `start_mfa_challenge_for` |
| `GET  /api/app/session/challenge/options` | WebAuthn assertion options for the pending challenge | `SessionChallengesController#passkey_options` |
| `POST /api/app/session/challenge/email_code` | Dispatch the email OTP for the pending challenge | `#send_email_code` |
| `DELETE /api/app/session` | Sign out → destroy the `Session` | `terminate_session` |
| `GET  /api/app/me` | Bootstrap: user, workspace, role, locale, feature flags, entitlements, setup/onboarding state, unread counts | new `MeSerializer` (see v1 `me#show`) |
| `POST /api/app/passwords` / `PUT …/:token` | Request reset / set new password | `PasswordsController` |
| `POST /api/app/registration` … `/complete` | 3-step signup → final step returns `{ token }` | `RegistrationsController` (each step) |
| `POST /api/app/invitations/:token/accept` | Accept invite → `{ token }` | `InvitationsController#accept` |
| `POST /api/app/oauth/native/exchange` | Redeem the `:native_session` one-time token → `{ token }` (the SPA/Capacitor twin of `sessions#native`) | `user.find_by_token_for(:native_session, …)` |
| `GET/PATCH /api/app/onboarding` (+ member actions) | Onboarding state + persona/suggestions/first-sync | `OnboardingController` |
| `GET/PATCH /api/app/setup/:id`, `POST …/dismiss` | Setup cards | `SetupController` |
| `POST /api/app/tours/:key/dismiss` | Dismiss a tour | `ToursController` |
| `GET/POST /api/app/ai_setup/:kind(/message/apply)` | Conversational setup (async, like Scout chat) | `AiSetupChatsController` |

## OAuth sign-in — the native handoff is already SPA-shaped

`OauthNativeHandoff` (`app/controllers/concerns/oauth_native_handoff.rb`) already does exactly
what Capacitor needs: OAuth runs in the **system browser** (no app cookie), and on success the
server mints `user.generate_token_for(:native_session)` and redirects to
`campbooks://oauth?token=…`. The SPA/Capacitor app intercepts the deep link and calls
`POST /api/app/oauth/native/exchange` (above) to swap the one-time token for a Session bearer.

**Salvage, don't rebuild:** `Oauth::State` (`native: true`, signed), `OauthNativeHandoff`,
`redirect_to_native`. **No OAuth provider-console changes** — reuses the existing
`/oauth/{gmail,zoho,microsoft}/callback` redirect URIs. The web SPA can use the same one-time
handoff (popup/redirect → deep link or `postMessage` → exchange) so web and native share one path.

⚠️ **MFA note:** native OAuth sign-in is *provider-MFA only* today (documented exception —
`complete_oauth_sign_in`). If the SPA must enforce app-level MFA on OAuth sign-in, that's a new
in-app challenge step, not a salvage.

## Read models / serializers needed

- `Api::App::MeSerializer` — the single bootstrap payload the SPA loads on launch (identity +
  workspace + role via `useActingRole()` on the client + flags + counts). Highest-traffic
  endpoint; design it deliberately.
- `Api::App::SessionSerializer` — `{ token, expires_at }` shape returned by every login path.

## Real-time

None for auth itself. But `me` should carry the initial unread/notification counts the
`notifications_<user_id>` channel then keeps live (see [`realtime.md`](realtime.md)).

## Open questions

- Token lifetime & refresh: `signed_id` can carry `expires_in`; do we want a refresh endpoint
  or just re-login on 401 for web, and a long-lived secure-storage token for native?
- MFA challenge transport: reuse the encrypted-cookie `pending_mfa` marker, or make `mfa_token`
  a `generates_token_for(:api_mfa_challenge)` so it's fully stateless/cookieless? (Token is
  cleaner for cross-origin — recommended.)
- Passkey/WebAuthn on native Capacitor: platform authenticator availability differs from web —
  confirm the ceremony works through the Capacitor WebView or needs a plugin.
