# Campbooks — Store Listing Copy (draft)

_Draft for review. Tweak freely — these are the exact strings we'll paste into App Store Connect
and Google Play Console. Character limits noted in [brackets]._

## Shared facts

| Field | Value |
|---|---|
| App name | **Campbooks** |
| Developer / seller | Not A Camp LDA |
| Copyright | © 2026 Not A Camp LDA |
| Marketing URL | https://campbooks.not-a-camp.com |
| Support URL | https://campbooks.not-a-camp.com/support |
| Privacy Policy URL | https://campbooks.not-a-camp.com/privacy |
| Bundle ID / applicationId | com.notacamp.campbooks |
| Primary category | Productivity |
| Secondary category | Business |
| Age rating | 4+ / Everyone (no objectionable content) |
| Price | Free |

---

## Apple App Store

**App Name [30]:** `Campbooks`

**Subtitle [30]:** `The inbox that sorts itself.`  _(matches the marketing-site hero — 29 chars)_

**Promotional text [170, editable without review]:**
`Bring your inbox and paperwork into one calm place. Campbooks reads your email and documents, sorts what matters, and keeps it all private and EU-hosted.`

**Keywords [100, comma-separated, no spaces wasted]:**
`inbox,email,documents,invoice,receipt,AI,small business,workspace,paperwork,scan,pdf,bookkeeping`

**Description [4000]:**
```
Campbooks is a calm, AI-assisted workspace for the email and documents that run a small business.

Connect your mailbox and Campbooks brings in your messages and attachments, uses AI to classify and surface what actually needs you, and gives you a clear place to review, approve, and file it — instead of a cluttered inbox.

WHAT YOU CAN DO
• Connect Zoho or Google email over secure OAuth
<!-- Held until Microsoft ships (gated behind ENABLE_MICROSOFT): "Connect Zoho, Google, or Microsoft email over secure OAuth" -->
• Let AI triage your inbox and pull out invoices, receipts, and key documents
• Review and approve documents with a clear, human workflow — nothing is filed behind your back
• Keep meetings in view with a built-in two-way calendar
<!-- Held until the Workflow engine ships (gated behind ENABLE_WORKFLOWS): • Automate repetitive steps with simple workflows -->
• Search everything in one place

PRIVATE BY DESIGN
• Your data is hosted in the European Union and never sold
• Sensitive credentials are encrypted at rest
• Optional AI runs under your own provider key — you choose the provider
• No advertising, no third-party tracking

SOURCE-AVAILABLE
Campbooks is free to self-host and source-available. Use our hosted cloud, or run it on your own server.

Campbooks is built by Not A Camp, a studio making honest tools for small businesses.

Note: Campbooks cloud is currently in closed beta — request access at https://campbooks.not-a-camp.com
```

**What's New (v1.0):**
`First release of Campbooks for iPhone and iPad: your AI-assisted inbox and document workspace, now in your pocket.`

---

## Google Play

**App title [30]:** `Campbooks`

**Short description [80]:**
`The inbox that sorts itself — email, invoices & documents, filed for you.`

**Full description [4000]:** _(same body as the Apple description above — Google allows light
formatting; keep the feature bullets)_

**Feature graphic:** 1024 × 500 PNG/JPG (no alpha). Needed for the listing — will produce in the
assets phase.

---

## Privacy / data disclosures — final answers

These map field-by-field to the two console forms. Derived from the verified data model
(account = name/email; user-connected mailbox content + documents; encrypted OAuth tokens + AI key;
basic operational logs; crash/error monitoring via GlitchTip). No ads, no cross-app tracking, no
data sold.

### Apple — App Privacy (App Store Connect → App Privacy)

- **Data used to track you:** **None.**
- **Data linked to you** (purpose: *App Functionality*; account fields also *Account Management*):
  - **Contact Info** → Name, Email Address
  - **User Content** → Emails or Text Messages (connected mailbox); Other User Content (documents &
    attachments the user brings in)
  - **Identifiers** → User ID
- **Data not linked to you:**
  - **Diagnostics** → Crash Data, Performance Data (error monitoring) — purpose: App Functionality
- **Everything else** (Location, Financial Info, Health, Device Contacts, Browsing History,
  Purchases, etc.): **Not Collected.**
- Note: device **Contacts** = the phone address book, which we do NOT access. Campbooks' in-app
  "contacts" are derived from the user's own email content, declared above as User Content.

### Google Play — Data safety

- **Does the app collect or share user data?** Collect: **Yes**. Share: **No** (see nuance below).
- **Is all data encrypted in transit?** **Yes** (HTTPS).
- **Can users request data deletion?** **Yes** → in-app (Settings → Account) and
  https://campbooks.not-a-camp.com/data-deletion.
- **Data types collected** (all: purpose *App functionality*; account also *Account management*;
  not shared; **not** "processed ephemerally"):
  - **Personal info** → Name, Email address — *collection required* (needed for an account)
  - **Messages** → Emails — *optional* (only if the user connects a mailbox)
  - **Files and docs** → documents & attachments — *optional*
  - **App info and performance** → Crash logs, Diagnostics — *optional*
- ⚠️ **One judgment call to confirm — third-party transfers.** When a user turns on AI features or
  connects a mailbox, content is sent to a provider **they** chose and control (their AI key, their
  mailbox over OAuth). Google's "sharing" generally **excludes** transfers made at the user's
  direction / to service providers acting on our behalf — so the recommended answer is **"data is
  not shared."** We disclose these user-directed transfers plainly in the Privacy Policy. Confirm
  you're comfortable representing it this way (I think it's correct and honest).

### Both stores
- Account creation is required to use the app → declare "users can create an account."
- Data **retention/deletion**: deleted on account deletion; backups purged within 30 days (matches
  `/data-deletion`).

---

## App Review prep (important for Apple)

- **Demo account:** App Review needs working credentials. Create a dedicated reviewer account on
  **production** (app.campbooks.not-a-camp.com) pre-loaded with a little sample email/document data,
  and put the email + password in App Store Connect → App Review Information. (The dev seed
  credentials won't work on production.)
- **Account-based app:** the app requires sign-in, so the demo account is mandatory or it gets
  rejected at first screen.
- **OAuth note for reviewers:** explain that mailbox connection uses the system browser
  (ASWebAuthenticationSession / Custom Tabs) and that the reviewer can evaluate the app with the
  demo account's existing data without connecting their own mailbox.
- **Guideline 4.2 (minimum functionality):** Hotwire Native apps can draw "this is just a website"
  scrutiny. Mitigations: native navigation/transitions are already in place; adding **push
  notifications** is the strongest signal of native value if we get a 4.2 push-back. Flagged for
  Phase 4.
- **Account deletion (5.1.1(v)):** ensure the in-app Settings → Account → Delete flow is reachable
  by the reviewer without special steps.

---

## Open decisions

- [x] Subtitle → `The inbox that sorts itself.` (matches marketing hero)
- [x] Primary category → Productivity (Business secondary)
- [ ] Store sign-up model — **deciding**: open sign-up vs sign-in-only in the app (sign up on web).
      Ties into App Review 4.8 (login services) + the reviewer demo account.
- [ ] Launch both platforms together, or whichever account verifies first
- [ ] Dedicated `support@` address vs current `support@example.com`
