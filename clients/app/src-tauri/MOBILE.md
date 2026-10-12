# Mobile and tablet builds (iOS, iPadOS, Android)

The Tauri shell already targets mobile (`lib.rs` has the `mobile_entry_point`, the crate builds
`staticlib`/`cdylib`), but the native projects are **not generated yet**: `gen/apple` and
`gen/android` are created by the commands below and should be committed when that task lands.
Tablets are the same iOS/Android targets; the UI is responsive down to 375px.

Run everything from `clients/app`.

## iOS / iPadOS (macOS only)

Prerequisites:

- Xcode (full app, not only the Command Line Tools), opened once to accept the license.
- `xcode-select -s /Applications/Xcode.app/Contents/Developer`
- CocoaPods: `brew install cocoapods`
- Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`
- An Apple developer team for device builds (`bundle.iOS.developmentTeam` in
  `tauri.conf.json`, or the `APPLE_DEVELOPMENT_TEAM` environment variable).

```sh
pnpm tauri ios init                  # generates src-tauri/gen/apple
pnpm tauri ios dev                   # simulator; add a device name for hardware
pnpm tauri ios dev --open            # open in Xcode instead
pnpm tauri ios build                 # .ipa for TestFlight / App Store
```

## Android

Prerequisites:

- Android Studio with: Android SDK Platform (API 34+), Platform-Tools, Build-Tools, NDK (Side by
  side), Command-line Tools, and CMake (the `aws-lc-rs` TLS backend builds C code).
- JDK 17+ (Android Studio's bundled JBR works).
- Environment:

  ```sh
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"  # macOS
  export ANDROID_HOME="$HOME/Library/Android/sdk"                                  # macOS
  export NDK_HOME="$ANDROID_HOME/ndk/$(ls -1 "$ANDROID_HOME/ndk" | tail -1)"
  ```

- Rust targets:
  `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`

```sh
pnpm tauri android init              # generates src-tauri/gen/android
pnpm tauri android dev               # emulator or connected device
pnpm tauri android build --apk       # or --aab for Play Store
```

## Things to settle in that task

- **Dev server reachability:** on a device the webview cannot reach `localhost`. `tauri
ios|android dev` sets `TAURI_DEV_HOST`, which `vite.config.ts` already honours (binds to that
  host, HMR on port 5174). The API URL for the Rust side is `CAMPBOOKS_API_URL` and must also
  be a LAN/HTTPS address, not `localhost`.
- **CSP:** `tauri.conf.json` allows only `ipc:` / `http://ipc.localhost` for `connect-src`;
  the dev CSP adds `ws://localhost:5173` for HMR, which needs the device host instead.
- **Tokens:** ADR 0004 — Keychain (iOS) / Keystore (Android) via the `keyring` crate, OAuth 2.1
  authorization code + PKCE in the system browser with a deep-link redirect
  (`tauri-plugin-deep-link`). Not implemented yet; see `src/auth.rs`.
- **Icons:** replace the placeholder icons with `pnpm tauri icon <1024px-source.png>`, which
  also generates the iOS and Android sets.
