# Track the identity module's changes

Module: `identity` (its API operations, the issuer's protocol and sign-in
endpoints, its admin operations, event handlers and schedule), with the
tracking plan (`tracking/`). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/326, part of epic
https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action: who did it, in what context,
and what the records it touched looked like then. Identity is where the
security-relevant actions happen: signing in and failing to, creating an
account, ending sessions, adding and removing ways in and passkeys, letting
apps in and taking that back, registering apps and rotating their secrets,
issuing and revoking tokens, and an administrator suspending an account.
None of it is tracked yet, so the warehouse and the admin event view cannot
answer "who signed in, how, and what changed on this account". Full coverage
becomes a CI failure with campbooks-cloud#332, so every operation, handler
and schedule of the module needs a Coverage row first.

## What Changes

- Tracking plan version 3 (`tracking/patches/0003-identity.yaml`): 27 new
  live events of `identity`, the snapshot records `identity.account`,
  `.session`, `.way_in`, `.passkey`, `.client` and `.consent`, their enums,
  and a Coverage row for each of the module's 25 mutating operations (23 in
  the API document, 2 in the admin document), the 4 reads that change
  state (the provider callback, the GET logout, starting a round trip, the
  authorization endpoint), 5 event handlers and the prune schedule.
- Every live event is tracked with `Tracker::track` in the transaction of
  its change, after the change, from the module's own rows read in that
  transaction; a no-op tracks nothing. A refused sign-in changes nothing, so
  its event is written in a transaction of its own.
- Sign-ins that happen before anyone holds a token (the provider callback,
  the new-account screen, passkeys, consent, the token and revocation
  endpoints, dynamic registration, logout) run as actor `anonymous`; their
  events name the account by id in the body.
- The protocol routes in the module's tests run in the request tracking
  context, as the server mounts them, and the hand-over of a mailbox grant
  (spawned off the request) keeps that context.
- A test per event reads it back from `kernel.tracked_events`.

No new endpoint, scope, migration, dependency or domain event. No change to
what any operation returns.

## Non-goals

- The other modules (#325, #327–#331) and the hard coverage gate (#332).
- The mailbox connections a round trip hands to the connectors module:
  tracked by `connectors` (#329).
- Tracking every token refresh or client-credentials exchange: routine
  renewals every few minutes per session or app (see design).
- Link and connect round-trip failures (`link_error`): the person is
  already signed in; only sign-in failures are tracked.
- Any personal data or content in events: no addresses, names, provider
  subjects, tokens, secrets, codes, credential ids, user agents, redirect
  URIs, metadata-document URLs or IP addresses.
