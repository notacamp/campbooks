# Design

## Decision (lead, security first): Option B

The desktop page policy matches the web one, narrowly: only
`style-src-attr 'unsafe-inline'` and `img-src 'self' data: blob: https:`
are added. The mail frame keeps every other guarantee, and nothing in it
can run.

What a hostile mail could do, layer by layer:

- **Script**: none. The sanitiser strips it; the sandbox has no
  `allow-scripts`; the frame's `default-src 'none'` and the page's
  `script-src 'self'` block inline script. With nothing running, Tauri's
  IPC (`window.__TAURI_INTERNALS__`, invoke, the credentialed
  `api_request`) is out of reach.
- **Forms**: no `allow-forms`, `form-action 'none'` on both policies.
- **Navigation**: no `allow-top-navigation`; the frame cannot be navigated
  to a remote page (`frame-src` falls back to `default-src 'self'`), a meta
  refresh included. A link asking for a new window is denied as a webview
  by the shell; a web or mail link opens in the system browser.
- **CSS exfiltration**: style sheets stay blocked (`style-src 'self'`
  intersected with the frame's policy stops inline `<style>`, `@import` and
  `<link>`); attribute styles load only through `img-src`, which is `data:`
  until Show images or a trust rule.
- **Remote loads before Show images**: none; the frame's `img-src data:`
  holds until the member asks or a rule trusts the sender.

Blast radius if the sanitiser misses something: the same as on the web.
The page itself only gains inline `style` attributes and `https:` images,
which matter only with an HTML-injection bug in the app's own page; script
stays `'self'` and IPC is untouched.

## Rejected

- **Option A: the frame on its own origin or custom scheme**, with its own
  strict policy. Strongest IPC isolation, but a cross-origin frame cannot be
  measured from outside: fitting, zoom, height and the mail's `@media` rules
  would need a script in the mail frame (`allow-scripts` with a measuring
  script posting its height) or a frame that scrolls on its own. Both are
  worse trades today, and it needs a credentialed protocol handler in Rust.
  Rejected for now; revisit only if script in the frame is ever needed.
- **Tauri's isolation pattern**: it wraps every IPC call in a hook but cannot
  tell which frame made it, so it does not separate the mail frame.
- **`blob:` / `data:` frame URLs**: they inherit the page policy like
  `srcdoc`; nothing gained.
- **A per-frame policy in Tauri**: there is none; `app.security.csp` covers
  the assets Tauri serves.

## Verifying

- `test:csp` (CI, WebKit): the frame from `lib/ui/safe-html/frame.ts`, under
  the desktop and web policies read from where they are defined, around the
  server's sanitised output for the #160 fixtures and around hostile markup
  no sanitiser would pass. It asserts styles and `data:` images render, zero
  requests before Show images and loads after, no script (`<script>`,
  `onerror`, `<svg onload>`, `javascript:`), no form post, no frame
  navigation or meta refresh, no `@import` / `<link>` / `<style> url()`,
  no plain `http:`, and a trusted sender's images with no tracker.
- The fixture snapshots are the sanitiser's output: a Rust test fails when
  they drift (`UPDATE_SNAPSHOTS=1` rewrites them).
- The shell's unit tests: link schemes, the capability (main window only, no
  remote URL, no opener grant), the policy's directives.
- Phase 2, the real shell with `tauri-driver`: campbooks-cloud#237.
