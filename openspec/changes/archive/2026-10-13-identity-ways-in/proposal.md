# Proposal

## Why

ADR 0014 makes every mailbox a person owns a way into the same Campbooks
account, linked only by proven ownership. Slice S1a built the issuer and
signing in with Google; S1b built the sign-in screens. A person still can't
add a second way in, can't connect another mailbox through the one front
door, can't remove a way in, and can't sign in with Microsoft or Zoho.

This change is **slice S2** (campbooks-cloud#79, parent #75):

- more ways in: link one while signed in, connect a mailbox (which is a way
  in by construction), reconnect a mailbox that needs re-authorisation, and
  remove a way in (never the last one);
- Microsoft (personal, work and school accounts, through `common`) and Zoho
  (every data centre) as sign-in providers;
- `connectors`' consent goes through `identity`, so `identity` is the single
  entry point for provider consent;
- Settings → **Ways to sign in**, and the sign-in screen offering Microsoft
  and Zoho once the server lists them.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/79

**Modules touched**
- **identity**: link tickets and purposes, Microsoft and Zoho round trips,
  `identity.start_link`, `identity.list_ways_in`, `identity.remove_way_in`,
  the consent front-door port, events `identity.way_in_added` and
  `identity.way_in_removed`.
- **kernel**:
  - `auth`: an ID token validated against a per-token issuer, with extra
    claims (Microsoft's `tid`, `xms_edov`);
  - `connections`: `ProviderGrant.extending`, `GrantIntake::reconnect_target`,
    and the `ConsentDoor` port (`identity` implements it);
  - `testing::TestIdp`: extra ID-token claims, to play Microsoft and Zoho.
- **connectors**: implements `reconnect_target`; `start_connection` becomes a
  delegate of `identity`'s front door; `complete_connection`, its
  `authorizations` table and its callback page are retired.
- **server**: registers the consent door.
- **clients/app**: Settings → Ways to sign in; Microsoft and Zoho on the
  sign-in screen; a dev-only fake Microsoft and fake Zoho for recordings.

No cross-module dependency is added: `connectors` reaches `identity` through
a kernel port, as `identity` already reaches `connectors`.

## What Changes

- `POST /api/v1/identity/links` (`identity.start_link`, `identity:write`)
  with purpose `link`, `connect` or `reconnect`; it returns a short-lived,
  single-use URL of `/auth/start/{provider}?link=…` that only a browser
  signed in to the same account can use.
- The callback links the provider identity to the signed-in account, or
  refuses with `way_in_belongs_to_another_account` (nothing merged or
  moved). For `connect` and `reconnect` the grant then goes to the
  connectors module at once, through the grant-intake port.
- `GET /api/v1/identity/ways-in` (`identity:read`) and
  `DELETE /api/v1/identity/ways-in/{id}` (`identity:write`), refused with
  `last_way_in`.
- Step-up: linking, connecting and removing need a sign-in in the last ten
  minutes (`step_up_required`); reconnecting does not.
- Microsoft sign-in (`/auth/start/microsoft`), its issuer bound to each
  token's own `tid`, its email never verified unless `xms_edov` says so.
- Zoho sign-in (`/auth/start/zoho`) for every data centre, the
  `accounts-server` checked against a fixed allowlist before any request.
- Offline access is asked for only when mailbox scopes are, so a sign-in
  with Microsoft or Zoho (no mail adapter yet) keeps no provider token.
- `connectors.start_connection` answers with `identity`'s front-door URL;
  `connectors.complete_connection` is removed.
- UI: Settings → Ways to sign in (list, add, remove with inline
  confirmation, reconnect), and Microsoft and Zoho on `/sign-in`.

## Non-goals

- Passkeys (S3, #80). The Ways to sign in page renders S3's passkeys section;
  `remove_way_in` counts passkeys through S3's `ways_in::lock_and_count`.
- Native sign-in, third-party clients, consent, DCR (S4, #81).
- Removing Rauthy (S5, #82).
- Mail adapters for Microsoft (Graph) and Zoho (#74): until they exist, those
  providers are ways in only, and `connect` is refused with
  `mailbox_unsupported`.
- Merging two accounts, or moving a way in from one account to another.
- Recent sessions and connected apps on the Ways to sign in page (S4).
- Renaming a way in.
