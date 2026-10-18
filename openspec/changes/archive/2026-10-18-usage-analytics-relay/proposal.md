# Proposal

## Why

The owner wants to know which screens of the app people use, in the
operator's existing Matomo. The obvious way, Matomo's JavaScript tracker,
would put a third-party script, served by a machine whose admin login faces
the internet, inside the app that reads people's mail, and would widen the
content security policy of both the web client and the desktop app. This
change counts screens without that: the app tells its own server which
screen it shows, and the server relays it to Matomo.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/241

## What Changes

- Modules touched: `system` (a new slice, `features/screen_views`, and a new
  resource type `system.screen_view`), the kernel (an `analytics` config
  section and an HMAC helper in `crypto`), the server (composes the relay)
  and the client (`lib/telemetry/screen-views`, wired in `app/providers`).
- New operation `system.report_screen_view`:
  `POST /api/v1/system/screen-views` with `{ "route": "/w/:workspaceId/inbox" }`,
  scope `system:read`, answering 204. Anything but a route template is
  refused with 400 `invalid_route` and never forwarded.
- New settings `CAMPBOOKS_ANALYTICS__MATOMO_URL`,
  `CAMPBOOKS_ANALYTICS__MATOMO_SITE_ID` and
  `CAMPBOOKS_ANALYTICS__MATOMO_VISITOR_KEY` (secret). Off unless all three
  are set; off, the operation does nothing and nothing leaves the server.
- The server relays each view through a bounded in-memory queue and one
  background worker to Matomo's HTTP tracking API, with no IP, no user id,
  no workspace and a visitor id that changes every UTC day.
- The app reports the route template after each navigation to another path,
  and nothing while Do Not Track or Global Privacy Control is on.
- New metric `campbooks.screen_views` by `outcome`.
- No new module and no cross-module dependency.

## Non-goals

- No Matomo (or any analytics) script in the web client or the desktop app,
  and no change to either content security policy.
- No events, goals, timings, titles, search terms or custom dimensions; no
  user id and no workspace in Matomo. Retention and active users come from
  the warehouse, not Matomo.
- No consent banner: nothing identifying is collected, and the browser's own
  privacy signals are honoured.
- No retry or persistence of views: a lost view is lost.
- The desktop bridge does not forward the webview's user agent (its views
  carry the bridge's).
