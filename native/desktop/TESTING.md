# Testing the desktop app

The Rails-side native contract is covered by the server test suite. The Tauri
shell itself is built and verified locally; the OAuth deep-link mapping has Rust
unit tests (`cargo test` in `src-tauri`).

## Prerequisites

See **Build & run → Prerequisites** in [README.md](README.md). In short: Rust
(stable) + the Tauri v2 CLI; on Linux the `libwebkit2gtk-4.1-dev` system deps.

---

## Dev — against a local server

```bash
# 1. Run the web app + worker (repo root)
bin/rails server
bin/rails solid_queue:start

# 2. Launch the desktop shell pointed at http://localhost:3000
cd native/desktop/src-tauri
cargo tauri dev
```

A window opens on the Campbooks sign-in page. Sanity checks that the native
treatment is active (i.e. the `Hotwire Native` UA is being sent and accepted):

- **No 406 / "browser not supported"** page (proves the modern-browser UA passes
  `allow_browser`).
- **No beta banner** and **no web topbar** chrome.
- The sign-in page shows **direct** "Continue with Google/Microsoft" buttons
  (the native variant), not the web redirect buttons.

---

## What works immediately vs. what needs credentials

| Flow | Status |
|---|---|
| Loading the app, browsing, email + password sign-in | ✅ works against any running server |
| OAuth sign-in / account-link deep-link handoff | ⚙️ needs provider OAuth credentials configured on the server |
| Code-signed / notarized installers | ⚙️ needs Apple Developer / Windows signing certs (see README → follow-ups) |
| Auto-update | ⚙️ not wired yet (README → Auto-update) |

### Verifying the OAuth handoff (once provider creds exist)

1. Click **Continue with Google** (or Microsoft/Zoho).
2. The provider page must open in your **default system browser**, *not* inside
   the app window. (If it opens in-window, `oauth::is_provider_host` isn't
   matching — check the host against `PROVIDER_HOSTS`.)
3. Complete the consent. The browser hits `…/oauth/*/callback`, which redirects
   to `campbooks://oauth?…`.
4. The OS hands the deep link back to the app, which navigates the window to
   `/session/native?token=…` (sign-in) or `/email_messages?inbox_settings=accounts`
   (account-link). You end up signed in / with the account linked.
   - **macOS**: the scheme is registered from `Info.plist` at build time — test
     with a built app (`cargo tauri build`), as `cargo tauri dev` may not have a
     registered bundle.
   - **Windows / Linux**: `cargo tauri dev` registers the scheme at runtime
     (`deep_link().register_all()`), so the handoff works in dev too.

> ⚠️ Use a **test account**. Per project policy, never send real email and avoid
> mutating a real mailbox while testing.

---

## Build verification

```bash
cd native/desktop/src-tauri
cargo test          # OAuth mapping unit tests
cargo tauri build   # host-OS installer → target/release/bundle/
```

Open the produced `.dmg` (macOS) / `.AppImage` or `.deb` (Linux) / `.msi`
(Windows) and confirm the app launches and loads production.

---

## Troubleshooting

- **406 / "browser not supported"** — the UA isn't being recognized as modern.
  Confirm `config::USER_AGENT` for your OS still carries a current Safari/Edge
  version token.
- **OAuth opens in-window** — the provider host isn't in `oauth::PROVIDER_HOSTS`;
  add it (and mirror it in the mobile configs).
- **Deep link does nothing** — on macOS, the scheme is only registered for a
  *built* app, not `cargo tauri dev`. Build and test the bundle.
- **Two windows open on deep link (Windows/Linux)** — the `single-instance`
  plugin must be registered **first** and built with `features = ["deep-link"]`.
