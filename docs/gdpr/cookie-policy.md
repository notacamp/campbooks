# Cookie policy — Campbooks

> ⚠️ Review-ready draft, not legal advice. This reflects the **actual** cookies the
> app sets (verified in code). Publish a user-facing version (web page + privacy
> policy section). Keep it in sync if tracking/analytics is ever added.

Campbooks uses the **minimum cookies needed to run the service**. We do **not** use
advertising, analytics, or third-party tracking cookies, and (since 2026-06-21)
the app self-hosts its fonts, so loading a page makes **no request to Google or
other third-party CDNs**.

## Cookies we set

| Cookie | Purpose | Type | Duration |
|---|---|---|---|
| `session_id` | Keeps you signed in (signed, `HttpOnly`, `SameSite=Lax`, `Secure` in production). Holds only a server-side session reference. | Strictly necessary | Persistent cookie; the **server-side session expires after 30 days of inactivity** |
| `_campbooks_session` | Rails session / CSRF protection | Strictly necessary | Session |

That's it. No `_ga`, no Plausible/PostHog/Mixpanel, no Meta/LinkedIn pixels, no
embedded third-party trackers.

## Do we need a consent banner?
Strictly-necessary cookies are **exempt from prior consent** under the ePrivacy
Directive. As long as the app sets **only** the essential cookies above and no
analytics/tracking, a consent banner is **not required** — a clear cookie policy
(this document, linked from the footer/privacy policy) suffices.

**This changes the moment any non-essential cookie or third-party tracker is
added** (analytics, a marketing pixel, an embedded widget that sets cookies, a
re-introduced font/CDN that sets cookies). At that point you must add a compliant
consent mechanism (granular, opt-in, no pre-ticked boxes) **before** those cookies
load. Treat that as a hard gate in code review.

## The marketing website (Docusaurus)
The static marketing site **self-hosts all fonts** (Inter, JetBrains Mono, Clash
Display) as of 2026-06-22 — no Google Fonts / Fontshare requests, so no visitor IP
reaches a third-party font CDN. The site sets no third-party cookies and makes no
third-party font requests.

Last updated: 2026-06-22.
