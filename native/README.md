# Campbooks native apps (Hotwire Native)

Thin native shells for **iOS** and **Android** that wrap the existing Campbooks
web app with [Hotwire Native](https://native.hotwired.dev/). Screens are the same
HTML the web serves; the native layer adds native navigation, the OAuth handoff,
and a path home to native screens/bridge components later.

> **These are scaffolds, authored but not compiled here.** They were written
> without a full Xcode / Android SDK available, so treat them as a working
> starting point: open them in Xcode / Android Studio, resolve dependencies, and
> expect to nudge a few SDK calls if the pinned SDK version differs. The
> Rails-side integration they depend on is implemented and tested.

## How it fits together

- The server detects native requests via `hotwire_native_app?` (turbo-rails;
  the WebView sends a `Hotwire Native` User-Agent) and hides the web topbar,
  adds a `hotwire-native` body class, and serves a path-configuration ruleset at
  **`/configurations/ios_v1.json`** and **`/configurations/android_v1.json`**
  (`ConfigurationsController`).
- Each app loads a **bundled** path configuration for first launch and the
  **remote** one so navigation rules can change without an app update.
- Set the server URL in **`ios/Campbooks/Config.swift`** and
  **`android/.../Config.kt`** (`BASE_URL`). Defaults to production
  (`https://app.campbooks.not-a-camp.com`).

## OAuth handoff (the interesting part)

Providers (Google/Microsoft/Zoho) reject embedded webviews, and a system browser
doesn't share the WebView's session cookie. So:

1. A `RouteDecisionHandler` (`OAuthRouteDecisionHandler` on each platform)
   intercepts navigations to provider hosts and opens them in a **real** system
   browser — `ASWebAuthenticationSession` (iOS) / **Chrome Custom Tabs**
   (Android).
2. The server builds the authorize URL with a **signed** `state` carrying
   `native: true` (+ the user id for account-link) — see `Oauth::State` and the
   `OauthNativeHandoff` concern. The provider redirects to our **existing**
   `/oauth/*/callback` (so **no OAuth provider console changes are needed**).
3. The callback finishes server-side and redirects to **`campbooks://oauth?…`**:
   - **sign-in** → with a one-time token. The app loads
     `/session/native?token=…` in the **main WebView**, which sets the session
     cookie there.
   - **account-link** → the app reloads the accounts screen (already linked).
4. The `campbooks://` redirect is caught by the auth session (iOS) or the
   `campbooks://oauth` intent-filter on `MainActivity` (Android).

Email + password sign-in needs none of this — it posts within the WebView.

## Build & run

### iOS (`ios/`)
Requires macOS + Xcode and [XcodeGen](https://github.com/yonsugihara/XcodeGen)
(the `.xcodeproj` is generated from `project.yml`, not committed).

```bash
brew install xcodegen
cd native/ios
xcodegen                 # generates Campbooks.xcodeproj
open Campbooks.xcodeproj
```
Then in Xcode: pick a Team (Signing & Capabilities) and Run. The Swift Package
`hotwire-native-ios` resolves on first build. To point at a local server, set
`Config.rootURL` to `http://localhost:3000` (an ATS localhost exception is
already in `Info.plist`).

### Android (`android/`)
Requires Android Studio.

```
Open native/android in Android Studio → let Gradle sync → Run.
```
For the emulator against a local server, set `Config.BASE_URL` to
`http://10.0.2.2:3000` (cleartext for `10.0.2.2` is already allowed in
`res/xml/network_security_config.xml`).

## Not done yet (follow-ups)
- **App icons & launch screens** — add real artwork (Xcode asset catalog /
  Android `Image Asset`); placeholders/defaults are used now.
- **Native bottom tabs** — v1 keeps the web `BottomNav` for tab switching.
- **Search / notifications / profile** in the native nav bar (the web topbar is
  hidden) — candidate for a Bridge component.
- **Push notifications.**
- **Signing & distribution** — Apple Developer + Google Play accounts,
  provisioning, TestFlight / Play internal testing.
- **Versions** — the Android build is pinned to a **verified-building** set for
  Hotwire 1.2.8: Gradle **8.9** (wrapper committed), AGP **8.7.2**,
  compileSdk/targetSdk **35**, Kotlin **2.3.0** (`compilerOptions` DSL). iOS uses
  SPM `from: "1.2.0"` (resolves to the latest stable). Bump as new Hotwire
  releases land. `app-debug.apk` has been built successfully from this scaffold.
