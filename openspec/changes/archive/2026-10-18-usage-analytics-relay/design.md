# Design

## Why a relay, not Matomo's tracker

Matomo's JavaScript tracker is code from another server running inside the
app that reads people's mail: whoever controls that server (or its admin
login, which faces the internet) controls a script with the app's privileges.
Loading it would also mean allowing that origin in the web client's
`script-src`/`connect-src` and in the desktop app's policy. The relay keeps
both policies as they are: the app talks only to its own server, through the
path every API call already takes (the web BFF, or the desktop bridge), and
the server is the only one that talks to Matomo.

## What the app sends

After each navigation whose path changed (TanStack Router's `onResolved`
with `pathChanged`), the app takes the deepest match's `fullPath`, the
router's template (`/w/$workspaceId/inbox/`), drops a trailing slash and
writes its parameters as `:name`, then posts `{ "route": … }` with the quiet
API client. It never reads the location, the search, the fragment or the
document title. When `navigator.doNotTrack === "1"` or
`navigator.globalPrivacyControl === true` it sends nothing. The send is not
awaited and its failure is swallowed: no retry, no queue in the browser.

## What the server accepts

The server does not trust the app. `route` must be `/`, or at most 8
segments of at most 32 bytes, 120 bytes in all, where each segment is a
literal of lowercase letters, `-` and `_` starting with a letter, or a
parameter `:name` of letters starting lowercase. There are no digits, so no
UUID, number or id with digits passes; no `?`, `#`, `%`, `.`, `@`, space or
capital, so no query string, fragment, encoded text, file, address or title
passes. Anything else is 400 `invalid_route` and nothing is queued. A
template is checked rather than matched against a list of the client's
routes, so the server does not have to ship with, and stay in step with,
every client's route table. The check runs whether analytics is on or off,
so the contract is the same on every instance and a client bug shows up in
development, not only where Matomo is set.

## Endpoint and access

- Operation `system.report_screen_view`, `POST /api/v1/system/screen-views`,
  scope `system:read` (which every Campbooks app already asks for at sign-in
  and already uses for its own error reports), through a token not bound to
  a workspace (`Unbound`).
- Resource type `system.screen_view`, owned by the account, action `create`,
  checked with `authz::require_account`: a client token is refused.
- Counted by the authenticated API's per-caller limits like every scoped
  operation (`kernel::rate_limit::api`, the write class); no new limiter.
- Data ownership: none. Nothing is stored in any schema, no event is
  published or consumed, no job is enqueued. No AI is involved.

## Queue and worker

`ScreenViewRelay` lives in the system module's `Deps` and reaches the
handler as an axum `Extension` on that one route. It holds a Tokio `mpsc`
channel of 1024 tracking URLs, created with its worker on the first view
(so composing the module for catalogues starts nothing). The handler only
`try_send`s: a full queue drops the view at once (`outcome=dropped`). The
worker sends one GET at a time with a 2 s connect and 3 s overall timeout;
any failure drops that view (`outcome=failed`), a 2xx counts as
`forwarded`. Errors are logged without the URL (it holds the visitor id and
user agent) and the request never waits on any of it.

## What Matomo receives

`GET <MATOMO_URL>/matomo.php` with exactly `idsite`, `rec=1`, `apiv=1`,
`url=<CAMPBOOKS_PUBLIC_URL><route>`, `action_name=<route>`, `cid`,
`send_image=0`, `ua` (the request's `User-Agent`, at most 256 characters,
when sent) and `lang` (the request's `Accept-Language` reduced to a locale
we ship, when there is one).

- **No IP.** No `cip`, so no `token_auth` is needed and the server holds no
  Matomo credential. Matomo records the server's address for every visit.
- **Visitor id.** The first 16 hex characters of
  HMAC-SHA256(`MATOMO_VISITOR_KEY`, `<account id>:<UTC date YYYY-MM-DD>`).
  The same account gets the same id all day, another id the next day, and
  another account another id; without the key it cannot be turned back into
  an account or linked across days. That gives daily unique users per screen
  and nothing more; retention and active users come from the warehouse.
- **Sent as `cid`, not `_id`.** Every visit comes from the server's one
  address, so the people behind it share a fingerprint (`config_id`). With
  `_id`, Matomo's default recognition can still match on that fingerprint
  and merge different people; `cid` is a forced visitor id, which Matomo
  accepts without `token_auth` when it is 16 hex characters, and when it is
  set Matomo recognises visits by the visitor id alone
  (`VisitorRecognizer::shouldLookupOneVisitorFieldOnly`). No instance-wide
  Matomo setting is needed, so the sites tracked with cookieless scripts
  keep their recognition.
- **Nothing else.** No `uid`, no `_id`, no custom dimensions or variables,
  no workspace id, no title.

## Configuration

`CAMPBOOKS_ANALYTICS__MATOMO_URL`, `CAMPBOOKS_ANALYTICS__MATOMO_SITE_ID` and
`CAMPBOOKS_ANALYTICS__MATOMO_VISITOR_KEY`, read by the kernel's config like
every other setting (the `CAMPBOOKS_` prefix and `__` nesting). All three,
or analytics is off: a blank value counts as unset, so a compose file can
pass them through with empty defaults. Startup refuses a URL that is not
http(s), a site id that is not a positive whole number and a key shorter
than 32 characters, naming the setting and never the key. The key is a
`SecretString`: redacted in `Debug`, never logged. Nothing in the repository
names an analytics host or a site id.
