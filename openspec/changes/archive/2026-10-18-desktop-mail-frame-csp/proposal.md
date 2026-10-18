# Proposal

## Why

On the desktop app, HTML mail renders unstyled and without images. The
reader's mail frame is a `srcdoc` frame, which inherits the page's
Content-Security-Policy, and a frame's own policy can only narrow it. The
desktop page's policy (`default-src 'self'`) therefore blocks every inline
`style` attribute, the image placeholders, `cid:` images and, after Show
images, remote images. The web app was fixed by #507; the desktop app was
not.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/199

## What Changes

- Touched: the desktop shell (`src-tauri`: page policy, main window, links)
  and the client's tests and recordings. The server is unchanged; a Rust
  test keeps the sanitised fixture snapshots in step.
- The desktop page policy gains exactly what the mail frame needs, as the
  web policy has it: `style-src-attr 'unsafe-inline'` and
  `img-src 'self' data: blob: https:`. It keeps `script-src 'self'`,
  `style-src 'self'` (never `'unsafe-inline'` on either), `object-src 'none'`,
  `base-uri 'none'`, `form-action 'none'`, `frame-ancestors 'none'`, and
  frames under `default-src 'self'`. No `http:`: plain-http remote images
  stay blocked on desktop. `devCsp` moves the same way (it keeps the
  `style-src 'unsafe-inline'` the Vite dev server already needed).
- The mail frame is unchanged: no `allow-scripts`, `allow-forms` or
  `allow-top-navigation`, the same meta policy, the same sanitiser.
- Links: the main window is built in Rust, and a link that asks for a new
  window is always denied as a webview; a web or mail link opens in the
  system's browser or mail app (the opener plugin, used from Rust only),
  anything else opens nowhere. The capability grants IPC to the main window
  only, never to a remote URL.
- CI: a WebKit test renders the frame the app builds under the desktop and
  web policies, read from `tauri.conf.json` and `web.rs`; the `tauri` job
  runs the shell's unit tests (links, IPC grants, the policy stays narrow).
- A recording scenario shows the #160 fixtures under the desktop policy in
  WebKit, then Show images.

## Non-goals

- No change to the frame, its sandbox or the sanitiser.
- No per-frame origin (Option A) and no Tauri isolation pattern; see design.
- The real-shell `tauri-driver` proof is a follow-up (campbooks-cloud#237).
