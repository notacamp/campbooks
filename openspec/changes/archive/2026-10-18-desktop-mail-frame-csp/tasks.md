# Tasks

## 1. Desktop shell

- [x] 1.1 Page policy (and `devCsp`): `style-src-attr 'unsafe-inline'` and `img-src 'self' data: blob: https:` only
- [x] 1.2 Main window built in Rust; a new-window request is denied as a webview; web and mail links open in the system browser (opener, Rust only)
- [x] 1.3 Unit tests: link schemes, IPC grants (no remote URL), the policy's directives; the `tauri` job runs them

## 2. Tests

- [x] 2.1 `frame.ts`: the frame document and sandbox, shared with tests and recordings
- [x] 2.2 `test:csp` in WebKit under the desktop and web policies
- [x] 2.3 Sanitised fixture snapshots, kept in step by a Rust test

## 3. Docs

- [x] 3.1 Recording scenario `desktop-mail-frame` (`--browser webkit`)
- [x] 3.2 Follow-up for the real-shell proof (campbooks-cloud#237)
- [x] 3.3 CHANGELOG
- [x] 3.4 Gates green in CI
