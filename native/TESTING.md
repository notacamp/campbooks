# Testing the native apps

The Rails side is covered by automated specs and was verified end-to-end on a
running server (see "Rails side" below). The **native shells must be built and
run on your Mac** — they can't be compiled in CI/sandbox environments without a
full Xcode / Android SDK. This guide gets you from zero to a running app.

## Prerequisites & disk

A working Android setup (Studio + SDK + one system image + Gradle caches) needs
**~10 GB free**; Xcode is **~12–15 GB**. Check first:

```bash
df -h ~          # need several GB free before installing either toolchain
```

Hardware virtualization (required by the Android emulator) is already supported
on this Mac (`sysctl kern.hv_support` → `1`).

---

## iOS — Simulator (no Apple Developer account needed)

1. Install **Xcode** from the Mac App Store, then:
   ```bash
   brew install xcodegen
   cd native/ios && xcodegen && open Campbooks.xcodeproj
   ```
2. Point at your server: in `Campbooks/Config.swift` set
   `rootURL = URL(string: "http://localhost:3000")!` (the Simulator shares your
   Mac's localhost). Make sure `bin/rails server` is running.
3. Pick a simulator (e.g. iPhone 16) and press **⌘R**.

The Swift Package `hotwire-native-ios` resolves on first build. An ATS exception
for `localhost` is already in `Info.plist`, so plain http to the dev server works.

To run on a **physical iPhone**: a free Apple ID gives a 7-day signing cert
(Xcode → Signing & Capabilities → add your Apple ID as a Personal Team).

---

## Android — Emulator

1. Install **Android Studio** (bundles the SDK, emulator, and AVD manager).
2. Open `native/android`, let Gradle sync (downloads the Hotwire + AndroidX deps).
3. **Tools → Device Manager → Create Device** (e.g. Pixel 8, a recent system
   image).
4. Point at your server: in
   `app/src/main/java/com/notacamp/campbooks/Config.kt` set
   `BASE_URL = "http://10.0.2.2:3000"` — `10.0.2.2` is the emulator's alias for
   your Mac's localhost. Cleartext to `10.0.2.2` is already allowed in
   `res/xml/network_security_config.xml`.
5. Press **Run ▶**.

CLI alternative once the SDK is installed:
```bash
cd native/android
./gradlew assembleDebug          # build the APK (also a pure compile check)
adb install app/build/outputs/apk/debug/app-debug.apk
```

---

## What works immediately vs. what needs credentials

| Flow | Status |
|---|---|
| Web UI in the native shell, native push/modal nav, path config | ✅ works as soon as it loads |
| Topbar hidden / `hotwire-native` body class | ✅ (the app sends a `Hotwire Native` User-Agent) |
| **Email + password** sign-in | ✅ works against the dev server |
| **Google / Zoho** OAuth (sign-in + mailbox/calendar connect) | needs `GOOGLE_*` / `ZOHO_*` client creds in the server env |
| **Microsoft** OAuth | needs an Entra app registered (`MICROSOFT_*`) — not set up yet |

So **email + password is the clean first test**. OAuth is wired end-to-end (it's
unit-tested), but exercising it needs the provider client IDs/secrets configured
server-side.

### Verifying the OAuth handoff (once creds exist)

1. Tap "Sign in with Google" → it should open a **real system browser**
   (ASWebAuthenticationSession on iOS, a Custom Tab on Android), not the embedded
   web view. If it opens inside the app's web view, the
   `OAuthRouteDecisionHandler` isn't matching the provider host — check
   `Config.oauthProviderHosts` / `Config.OAUTH_PROVIDER_HOSTS`.
2. Complete the provider login. The server redirects to `campbooks://oauth?…`.
3. The app should catch that scheme and land you signed-in (sign-in) or back on
   the accounts screen (connect). If nothing happens after the browser closes:
   - **iOS**: confirm the `campbooks` URL scheme is in `Info.plist`.
   - **Android**: confirm the `campbooks://oauth` `<intent-filter>` is on
     `MainActivity` and the activity is `launchMode="singleTask"`.

---

## Troubleshooting

- **Gradle can't find `dev.hotwire:core:<v>`** — bump to the latest published
  version (`app/build.gradle.kts`); check
  https://github.com/hotwired/hotwire-native-android/releases and Maven Central.
- **SPM can't resolve hotwire-native-ios** — File → Packages → Reset Package
  Caches in Xcode.
- **App loads but is blank / "cannot connect"** — wrong server URL: simulator
  uses `localhost`, Android emulator uses `10.0.2.2`, a physical device needs
  your Mac's LAN IP (and the server bound to `0.0.0.0`).
- **Stuck on the Hotwire loading spinner forever** — navigation failed, almost
  always a path-config `uri` with no matching fragment. Set
  `Hotwire.config.debugLoggingEnabled = true` and watch logcat: a
  `navigateToLocation … No destination found / No fallback destination found`
  line means the page's `uri` (from `config/hotwire/path_configuration.json`)
  doesn't map to a registered fragment. Modal pages should use the default web
  fragment (`hotwire://fragment/web`) with `context: modal`, **or** register
  `HotwireWebBottomSheetFragment` and use its real `hotwire://fragment/web/modal/sheet`
  uri — not an invented one. (A truly offline device can also hang, since the
  importmap eager-loads tiptap/marked from esm.sh/jsdelivr; but that needs *zero*
  internet to trigger.)
- **Cleartext blocked on Android** — only `10.0.2.2`/`localhost` are allowed by
  `network_security_config.xml`; production is HTTPS.
- **External (non-OAuth) links** open in the system browser by design (the
  built-in Safari/Custom-Tab route handlers).

---

## Rails side (already automated)

These run anywhere and gate the server behavior the apps depend on:

```bash
bundle exec rspec \
  spec/services/oauth/state_spec.rb \
  spec/requests/configurations_spec.rb \
  spec/requests/native_session_spec.rb \
  spec/requests/hotwire_native_chrome_spec.rb \
  spec/requests/oauth/microsoft_spec.rb
```

They cover the signed `Oauth::State`, the path-config endpoint, the
`/session/native` token exchange, native-vs-web chrome, and the native OAuth
callbacks (sign-in token + cookie-less account-link via signed state).
