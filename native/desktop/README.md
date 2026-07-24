# Campbooks desktop app (Tauri)

A thin native **desktop** shell for **macOS, Windows and Linux** that wraps the
existing Campbooks web app with [Tauri v2](https://v2.tauri.app/). It's the
desktop twin of the Hotwire Native iOS/Android shells in `../ios` and
`../android`: screens are the same HTML the web serves, loaded in the OS webview
(WKWebView / WebView2 / WebKitGTK); the native layer adds the window, the OAuth
handoff, and the `campbooks://` deep-link path home.

> **Pure-Rust shell — no JS frontend.** The web app is loaded from a remote URL
> and is completely Tauri-unaware. Every native concern (sending the native
> User-Agent, opening provider pages in the system browser, catching the OAuth
> deep link) is handled in Rust under `src-tauri/src/`. Bundles are ~3–6 MB
> (except the Linux AppImage, which embeds WebKitGTK, ~70 MB).

## How it fits together

- The server already detects native clients via `hotwire_native_app?` (turbo-rails;
  it matches `Hotwire Native` in the User-Agent) and hides the web topbar, drops
  the beta banner, adds a `hotwire-native` body class, renders **direct** provider
  sign-in links, and bakes `native: true` into the OAuth state. The desktop
  webview sends a per-OS UA ending in `… Campbooks-Desktop Hotwire Native`
  (`src-tauri/src/config.rs`), so it gets exactly that treatment — and because the
  web topbar is `lg:hidden`, nothing is lost on a desktop-sized window (the left
  `NavRail` carries search / notifications / profile).
  - The UA is a **real, modern** browser string with the marker appended, because
    the Rails app guards every request with `allow_browser versions: :modern` — a
    bare token would be rejected with 406.
- The window loads `http://localhost:3000` in debug builds and
  `https://app.campbooks.not-a-camp.com` in release builds
  (`config.rs`, switched on `#[cfg(debug_assertions)]` — the same dev/prod
  convention as `ios/Campbooks/Config.swift`).

## OAuth handoff (the interesting part)

Providers (Google/Microsoft/Zoho) reject embedded webviews, and the system
browser doesn't share the webview's session cookie. So (mirroring the mobile
`OAuthRouteDecisionHandler`):

1. `on_navigation` (`src-tauri/src/lib.rs`) intercepts navigations whose host is
   an OAuth provider (`oauth::PROVIDER_HOSTS`, kept in sync with the mobile
   `Config.oauthProviderHosts`) and opens them in a **real** system browser via
   the `opener` plugin, cancelling the in-window navigation.
2. The server builds the authorize URL with a **signed** `state` carrying
   `native: true` (see `Oauth::State` + the `OauthNativeHandoff` concern). The
   provider redirects to our **existing** `/oauth/*/callback` (so **no OAuth
   provider-console changes are needed**).
3. The callback finishes server-side and redirects to `campbooks://oauth?…`. The
   `deep-link` plugin hands that URL to our handler, which navigates the window
   (`oauth::resolve`):
   - **sign-in** (`flow=signin&token=…`) → loads `/session/native?token=…`, which
     plants the session cookie in the webview.
   - **account-link** (`flow=connect`) → reloads `/email_messages?inbox_settings=accounts`
     (already linked server-side).
   - `status=error` / `add_sign_in` / unknown → no-op (stay put).
4. The `single-instance` plugin (with the `deep-link` feature) ensures that on
   Windows/Linux — where the OS launches a fresh process for the deep link — the
   URL is forwarded to the already-running window instead of opening a second one.

Email + password sign-in needs none of this — it posts within the webview.

## Build & run

### Prerequisites

- **Rust** (stable, ≥ 1.77) via [rustup](https://rustup.rs).
- **Tauri CLI v2**: `cargo install tauri-cli --version "^2"` (or the prebuilt
  `npm install -g @tauri-apps/cli@^2`).
- **Linux only**: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev
  patchelf libxdo-dev libssl-dev build-essential` (see `.github/workflows/desktop-release.yml`).
- **macOS**: Xcode Command Line Tools. **Windows**: MS C++ Build Tools + WebView2
  (pre-installed on Win 10/11).

### Run against a local server

```bash
bin/rails server                 # in the repo root — web app on :3000
cd native/desktop/src-tauri
cargo tauri dev                  # opens the window pointed at http://localhost:3000
```

### Build installers

```bash
cd native/desktop/src-tauri
cargo tauri build                # → target/release/bundle/{dmg,macos,…}
```

`cargo tauri build` only emits installers for the **host** OS — Windows and Linux
artifacts are produced by CI (`.github/workflows/desktop-release.yml`, a
macОS/Windows/Linux matrix). The icon is a glassy rounded squircle (desktop is
the only platform that doesn't round/mask the icon itself) sourced from
`native/assets/desktop-appicon.svg`; to regenerate the set:
`rsvg-convert -w 1024 -h 1024 ../../assets/desktop-appicon.svg -o ../../assets/desktop-icon-1024.png && cargo tauri icon ../../assets/desktop-icon-1024.png` (then delete the `icons/android` + `icons/ios` subdirs it also emits).

## Releasing

Push a **`desktop-v*`** tag (or run the `desktop-release` workflow manually) to
build the cross-platform matrix and draft a GitHub Release with the installers.

> ⚠️ The desktop app versions **independently** from the web app — its version
> lives in `src-tauri/tauri.conf.json`. Do **not** use a `vX.Y.Z` tag: those
> drive `publish-image.yml` → a production deploy.

## Auto-update

On launch the app checks the updater endpoint and, if a newer **signed** release
exists, shows a native "Update available → install & restart" prompt
(`check_for_updates` in `lib.rs` — Rust-side, since the webview is the remote app).

- **Signing:** updates are signed with a minisign keypair. The **public** key is
  in `tauri.conf.json` (`plugins.updater.pubkey`); the **private** key is the
  `TAURI_SIGNING_PRIVATE_KEY` repo secret (never committed; regenerate with
  `tauri signer generate`).
- **Endpoint:** `plugins.updater.endpoints` → a stable
  `https://files.not-a-camp.com/desktop/latest.json`. The release workflow builds
  the updater artifacts (`createUpdaterArtifacts: true`) and attaches `latest.json`
  to the GitHub release; **after each release, copy that `latest.json` to the files
  bucket** (`desktop/latest.json`) so installed apps see the new version. (GitHub's
  repo-wide `releases/latest` can't be used directly — desktop builds are
  pre-releases sharing the repo with the app's own `vX.Y.Z` releases.)
- The prompt only fires for a version **newer** than the running one, so it shows
  up after the *next* release ships.

## Not done yet (follow-ups)

- **Code signing** — macOS Developer ID + notarization (`APPLE_*` secrets; the
  release workflow is wired for them) and Windows Authenticode / Azure Trusted
  Signing (needs a Windows runner + cert). v1 ships **unsigned** (Gatekeeper /
  SmartScreen friction).
- **App-store submissions** — Mac App Store, Microsoft Store.
- **Native menu polish** — Tauri provides a default menu (Cmd+Q / copy-paste);
  refine if needed.
- **Marketing download page** — link the release assets.
