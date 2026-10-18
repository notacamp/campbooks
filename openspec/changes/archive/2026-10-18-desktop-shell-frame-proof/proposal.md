## Why

#199 made desktop HTML mail safe with a narrow page policy, and proved the
mail frame's policy in WebKit on CI (`test:csp`), plus Rust tests for link
opening and IPC grants. It never ran the real Tauri shell
(campbooks-cloud#237, phase 2). The #550 review added one more check: on
macOS, wry's navigation policy has no main-frame check, so a subframe
navigation could reach `on_navigation` and `open_outside`.

## What Changes

- A CI job, `tauri-shell`, runs a debug build of the desktop app with
  `tauri-driver` and WebKitWebDriver under xvfb. The app points at a mock
  API serving one conversation whose HTML mail is hostile. In the mail
  frame's realm, the job checks:
  - no `__TAURI_INTERNALS__` or `__TAURI__` exists (the app's page has
    one, as a control);
  - no script, `onerror` or `<svg onload>` ran, and no invoke reached the
    API;
  - inline styles apply, and nothing remote loads before Show images;
  - a `target=_blank` link opens no new webview and reaches the system
    opener alone;
  - a meta refresh or a nested iframe never reaches the system opener.
- A debug build records the links it hands to `open_outside` in
  `CAMPBOOKS_DEV_OPEN_OUTSIDE_LOG` instead of opening them, so the test can
  see them. Release builds never read the variable.
- `pnpm test:tauri` runs it locally on Linux.

## Impact

- Spec: inbox-ui (HTML mail on the desktop app).
- CI: a new parallel job, not a required check.
- Code: the debug-only hook in `src-tauri/src/links.rs`; the test, its
  Vitest config, a script, and knip and tsconfig entries.
