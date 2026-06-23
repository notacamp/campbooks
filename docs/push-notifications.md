# Push notifications (native iOS + Android)

Native push for the Hotwire Native apps. **Hotwire Native does no push itself** —
it's a `WKWebView`/`WebView` shell, and `WKWebView` can't use Web Push. So push
is plain native OS code (APNs on iOS, FCM on Android) talking to a shared Rails
delivery pipeline. (Browser/web push is intentionally **not** built — see the
"why not web push" note at the bottom.)

## How the pieces fit

```
  device (native code)              Rails                       Apple / Google
  ────────────────────             ───────                      ──────────────
  ask permission
  register w/ OS  ──token──▶  POST /device → Device row
                                      │
  (something happens) ─────▶  Notification.notify(...)
                                      │ after_create_commit (skips quiet tiers)
                                      ▼
                              PushDeliveryJob
                                ├─ ios  → Push::ApnsSender ──▶ api.push.apple.com ──▶ device
                                └─ andr → Push::FcmSender  ──▶ fcm.googleapis.com  ──▶ device
                                      │
                              prune Device on dead-token reply (410 / UNREGISTERED)
  tap notification ───────────────────────────────────────▶ deep-link into the web view
```

### Server pieces (built + tested)

| File | Role |
|------|------|
| `app/models/device.rb` | one row per app install (`platform` + push `token`); `Device.register!` upserts by token |
| `app/controllers/devices_controller.rb` | authenticated `POST /device` (register) + `DELETE /device` (remove by token) |
| `app/services/push.rb` | reads `APNS_*` / `FCM_*` env; `apns_configured?` / `fcm_configured?` gate everything |
| `app/services/push/apns_sender.rb` | one push to one iOS device via APNs (HTTP/2, token `.p8` auth) |
| `app/services/push/fcm_sender.rb` | one push to one Android device via FCM HTTP v1 |
| `app/jobs/push_delivery_job.rb` | fans a `Notification` out to the user's devices; prunes dead tokens |
| `Notification#enqueue_push_delivery` | `after_create_commit` hook; pushes only toast-worthy tiers (`action_required`, `awaiting`) |

**No setup → no-op.** When a provider's env vars are absent, that platform is
skipped, so the app boots and runs locally with zero push config.

### Native client pieces (not yet implemented)

The Swift/Kotlin code that requests permission, registers the token, posts it to
`/device` via a Hotwire bridge, and deep-links on tap is the next step. It can't
be tested until the setup below is done and a real device is available. **Full
checklist: see [What's left (TODO)](#whats-left-todo) at the bottom.**

---

## Setup you need to do

Two providers, independent — do iOS, Android, or both. Both require accounts only
**you** can create.

### A. Apple Push Notification service (APNs) — iOS

> Requires the **Apple Developer Program** ($99/yr). Bundle ID: `com.notacamp.campbooks`.

1. **App ID + capability.** [developer.apple.com](https://developer.apple.com) →
   Certificates, IDs & Profiles → **Identifiers** → your `com.notacamp.campbooks`
   App ID → enable **Push Notifications**.
2. **APNs auth key (.p8).** Keys → **+** → check **Apple Push Notifications service
   (APNs)** → Continue → Register → **Download** the `.p8`.
   ⚠️ You can only download it **once**. Note the **Key ID** shown next to it.
3. **Team ID.** Top-right of the portal (or Membership page) — a 10-char string.
4. **Drop the key in place** (gitignored):
   ```
   config/credentials/apns.p8
   ```
5. **Set env** (`.env` locally, prod `.env` on the server):
   ```
   APNS_KEY_ID=ABC123DEFG
   APNS_TEAM_ID=1234567890
   APNS_BUNDLE_ID=com.notacamp.campbooks
   APNS_KEY_PATH=config/credentials/apns.p8
   APNS_ENVIRONMENT=development   # see gotcha
   ```

> **`APNS_ENVIRONMENT` gotcha — match the build, not the Rails env.** APNs has two
> separate servers. A **debug build run from Xcode** gets a *sandbox* token →
> `development`. A **TestFlight / App Store build** gets a *production* token →
> `production`. Wrong value = `BadDeviceToken`. So: `development` while testing a
> debug build on your device; `production` in the real deployed app.

### B. Firebase Cloud Messaging (FCM) — Android

> Free. Package name: `com.notacamp.campbooks`.

1. **Project.** [console.firebase.google.com](https://console.firebase.google.com)
   → create a project (or reuse one).
2. **Android app.** Add app → Android → package `com.notacamp.campbooks` →
   download **`google-services.json`**. This one is the **client** file → it goes
   into the Android app at `native/android/app/google-services.json` (added with
   the native client code, not the server).
3. **Server credential.** Project settings → **Service accounts** → **Generate new
   private key** → downloads a JSON. This is the **server** file (don't confuse it
   with `google-services.json`). Drop it in place (gitignored):
   ```
   config/credentials/fcm-service-account.json
   ```
4. **Project ID.** Project settings → General → Project ID.
5. **Set env:**
   ```
   FCM_PROJECT_ID=campbooks-12345
   FCM_CREDENTIALS_PATH=config/credentials/fcm-service-account.json
   ```
6. Ensure the **Firebase Cloud Messaging API (V1)** is enabled (Cloud console →
   APIs; usually on by default).

---

## Verifying (once configured + a device is registered)

The senders accept any `Device`, so you can test from a console once a real device
has registered a token (i.e. after the native client exists):

```ruby
device = User.find_by(email_address: "you@…").devices.last

# Direct, low-level — returns :ok / :invalid / :error:
Push::ApnsSender.new.deliver(device, title: "Test", body: "Hello from APNs")
Push::FcmSender.new.deliver(device,  title: "Test", body: "Hello from FCM")

# End-to-end through the real pipeline (enqueues PushDeliveryJob):
Notification.notify(user: device.user, category: :system,
                    priority: :action_required, title: "Test push")
```

Check readiness anytime: `Push.apns_configured?` / `Push.fcm_configured?`.

### Testing notes

- **iOS needs a real device.** The Simulator can't obtain a real APNs token. Build
  a debug build onto a device → it registers → use `APNS_ENVIRONMENT=development`.
- **Android** push works in an emulator **with Google Play services** (or a real device).
- Only `action_required` + `awaiting` notifications push; quiet `activity` / `ai_reply`
  tiers are intentionally silent (mirrors the in-app toast rule).

### Production

Set the `APNS_*` / `FCM_*` env vars in the prod `.env`, and make the credential
files (`apns.p8`, `fcm-service-account.json`) present at their configured paths in
the container. They're gitignored; the deploy rsync currently copies
`config/credentials/` into the build context, so they'd be baked into the image —
fine to start, but consider mounting them as secrets instead of baking if that
matters to you.

---

## What's left (TODO)

Server pipeline is done + tested (2026-06-21). Everything below is still needed
to actually deliver a push to a phone.

### Required — no push works without these

- [ ] **Provider setup** (you, external) — Apple Developer + APNs `.p8`; Firebase
      project + service-account JSON. See "Setup you need to do" above. **Blocks
      all testing.**
- [ ] **iOS client** (`native/ios`, Swift):
  - [ ] Add the **Push Notifications** capability/entitlement to the Xcode project
  - [ ] Request permission (`UNUserNotificationCenter.requestAuthorization`) + `registerForRemoteNotifications()`
  - [ ] `didRegisterForRemoteNotificationsWithDeviceToken` → hex-encode → `POST /device` (platform `ios`)
  - [ ] Tap handling (`userNotificationCenter(_:didReceive:)`) → route the payload `url` into the `Navigator` (deep-link)
  - [ ] Foreground presentation (`willPresent`) so notifications show with the app open
  - [ ] `DELETE /device` on sign-out
- [ ] **Android client** (`native/android`, Kotlin):
  - [ ] Add the Firebase SDK + `google-services.json` + the gradle google-services plugin
  - [ ] `FirebaseMessagingService`: `onNewToken` → `POST /device` (platform `android`); handle `onMessageReceived`
  - [ ] `POST_NOTIFICATIONS` runtime permission (Android 13+) + a notification channel
  - [ ] Tap handling → deep-link into the WebView (intent extra → `Navigator`)
  - [ ] Remove the token on sign-out
- [ ] **The bridge** — how the native token reaches the authenticated server: a
      Hotwire `BridgeComponent` (Swift/Kotlin) ↔ a Stimulus controller that
      `fetch`-POSTs to `/device` with the CSRF token. Pin
      `@hotwired/hotwire-native-bridge` in importmap if used.
- [ ] **A real iOS device** to test end-to-end (the Simulator can't get a token).

### Polish / gaps (push works without these, but worth doing)

- [ ] **App icon badge** — nothing sets it yet. Compute the user's unread count and send `badge:` (APNs) / handle on Android.
- [ ] **Localized push copy** — the payload is the notification's stored `title`/`body`, rendered in whatever locale created it. Render in the recipient's locale (like the mailer's `with_recipient_locale`) if it matters.
- [ ] **Per-channel preference** — push currently mirrors the in-app toast tiers. Add a `notify_push` column to `NotificationPreference` (alongside `notify_in_app`/`notify_email`) + a Settings toggle if users should be able to opt out of push specifically.
- [ ] **Device management UI** — a Settings list of registered devices with a "remove" button.
- [ ] **Collapse / dedup** — set `apns-collapse-id` / FCM `collapse_key` so grouped notifications replace rather than stack. (Note: grouped bumps don't re-push today — only `create` does.)
- [ ] **Transient-error retry** — `PushDeliveryJob` is best-effort (its per-device `rescue` swallows failures). Add a retry for provider 5xx/timeouts if delivery reliability matters.
- [ ] **Prod secret delivery** — decide bake-into-image vs mount-as-secret for `apns.p8` + the FCM JSON (see the Production note above).

## Why not web (browser) push?

Web push (service worker + VAPID) targets people using Campbooks in a **browser** —
a different audience from the native apps, and one the native `WKWebView` can't use
anyway. It was deliberately scoped out in favour of native-only. If browser push is
wanted later, it reuses most of this pipeline (the `Notification` hook, the job
skeleton, dead-token pruning) with a third sender + a `web` platform on `Device`.
