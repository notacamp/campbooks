# Track the web BFF's changes

Module: the web backend-for-frontend, `bff` (`crates/server/src/bff/`: its
sign-in, session, logout and proxy endpoints, its event handlers and
schedule), with the tracking plan (`tracking/`) and its tool
(`crates/tracking-plan`). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/327, part of epic
https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action: who did it, in what context,
and what the records it touched looked like then. The BFF holds the web
app's sessions: it starts one when a person signs in on the web, and ends
it when they sign out, when the identity provider says so, when they sign
out everywhere or are suspended, and when it finds the session can no
longer be trusted or used (a copied cookie, a refused refresh). None of it
is tracked yet, so the record cannot answer "how did this web session end".
Its proxies (`/bff/api/*`, `/bff/admin/*`) forward to API operations that
track their own events: those events must name the operation reached and
the person, not the proxy. Full coverage becomes a CI failure with
campbooks-cloud#332, so every operation, handler and schedule of the BFF
needs a Coverage row first.

## What Changes

- Tracking plan version 4 (`tracking/patches/0004-bff.yaml`): 3 new live
  events of `bff` (`web_session_started`, `web_session_ended`,
  `web_sessions_ended`), the enums of how a web session ends, and a Coverage
  row for each of the BFF's 10 mutating operations, the 3 reads that change
  state (`login`, `callback`, `session`), 2 event handlers and the prune
  schedule.
- The plan tool knows `bff` as a platform module, like `kernel` and
  `server`: its events are generated into the kernel's `tracking.gen.rs` and
  tracked from `crates/server/`.
- Every web session's start and end is tracked with `Tracker::track` in the
  transaction that writes or deletes the session row (the store opens it);
  ending a session that is already gone tracks nothing. Events carry the
  session id, the account id and the cause only.
- The two handlers end an account's web sessions in the job's transaction
  and track how many ended.
- Tests read every event back from `kernel.tracked_events`, and show that a
  call through `/bff/api/*` or `/bff/admin/*` is tracked in the operation it
  reaches, as the person or the administrator, and that the handler a
  suspension causes runs on the administrator's behalf with the request's
  correlation.

No new endpoint, scope, migration, dependency or domain event. No change to
what any endpoint returns.

## Non-goals

- The other modules (#325, #326, #328–#331) and the hard coverage gate
  (#332).
- Tracking every request through the proxies: the operation reached tracks
  its own events.
- A session that times out (idle or absolute end): it ends by the clock,
  with no action of anyone and nothing observing that moment; the prune
  deletes such rows later, in batches, without ids.
- Refused callbacks (unknown or replayed state, an error from the
  provider, a code or ID token that does not verify): nothing changes, the
  identity provider tracks the sign-in's own outcome (`identity`), and
  tracking them would let anyone flood the outbox.
- Token refreshes and cookie rotations: routine renewals of a live session.
- Any personal data or content: no name, address, provider subject that is
  not a Campbooks account id, IdP session id, token, user agent, URL or IP
  address.
