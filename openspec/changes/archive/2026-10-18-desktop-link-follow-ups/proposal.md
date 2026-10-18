## Why

The independent review of #550 (campbooks-cloud#199) left four follow-ups
(campbooks-cloud#243): the desktop app treated `tauri.localhost` as its own
pages on every platform, although on macOS and Linux `*.localhost` is the
machine itself, where a local process could serve a look-alike page into the
main window (no IPC: a phishing risk only); mail links were re-encoded with
`+` for spaces, which mail apps show as a plus (RFC 6068); the release-build
exclusion of the dev server was untested; and the WebKit check of the mail
frame had no positive control and copied the frame's attributes.

## What Changes

- `tauri.localhost` is the app only on Windows and Android (`Build`, tested
  for both platforms on any CI host).
- A mail link keeps its fields as written, still percent-encoded; `attach=`
  and other fields are still dropped.
- The dev-server gate moves into the tested function (`Build.debug`).
- The WebKit test takes the sandbox and the referrer policy from `frame.ts`
  (`REFERRER_POLICY` is new, also used by the frame component) and adds a
  control with `allow-scripts` and no policy that must see a script run.

## Impact

- Spec: inbox-ui (HTML mail on the desktop app).
- Code: `src-tauri/src/links.rs`, `lib.rs`, `lib/ui/safe-html`,
  `test/webkit/frame-csp.webkit.ts`. Nothing changes on screen.
