# Design

## Context

What the backend already offers (#310–#313):

- `POST …/connections/imap/discover` (`connectors.discover_imap`, scope
  `connectors:write`, step-up): an address → `settings` (IMAP and SMTP
  servers, `username_hint`, `source`, `provider_hint` (the bundled ISPDB id:
  `yahoo.com`, `aol.com`, `me.com`, `gmx.com`, `gmx.net`, `web.de`,
  `yandex.ru`, `messagingengine.com`…), `password`: `app_password` |
  `account_password` | `unknown`), `use_provider` (`google`, `zoho`),
  `not_supported_yet` (`microsoft`) or `not_found`.
- `POST …/connections/imap` (`connectors.connect_imap`, `connectors:write`,
  step-up): signs in to both servers, then creates the connection.
  `address_verified` is true only when the username is the address.
- An IMAP send refuses an unverified address (`address_not_verified`,
  `rejected`).
- The Mailboxes screen lists `identity.list_ways_in`'s `mailbox_providers`
  (OAuth sign-in upstreams only, never `imap`) as Connect buttons.

## Decisions

### 1. The flow: a dialog that asks one thing at a time

| | Where the connect flow lives |
|---|---|
| A long form on the page (address, password, servers) | every field at once, most of them useless for Gmail or Outlook |
| A separate page per step (`/settings/mailboxes/add/…`) | the address would travel in the URL (history, logs) or in storage |
| **A dialog from "Add a mailbox", one step after another** (chosen) | one question per view (the product's rule for setup flows), Esc closes it with nothing kept, focus trapped, a bottom sheet on a phone; the design system's `Dialog` already does all of it |

Steps: **address** (one field, Continue ↵) → discovery →

- `use_provider` google / zoho: that provider's button (the existing front
  door, `identity.start_link` with `connect`), with "Also use this account to
  sign in", as before; a provider this instance does not offer says so;
- `not_supported_yet` microsoft: "Outlook is coming soon" and nothing to
  press but Back;
- `settings`: the **app password** step: the provider's how-to, the
  password field, Connect ↵, and "Advanced" (a disclosure, closed) prefilled
  from discovery;
- `not_found`: the same step with Advanced open and empty servers (the
  username prefilled with the address).

Then **checking** (when `address_verified` is false) → **verified** /
**not verified** (Try again), or straight to **connected**. Back (from the
second step) and Esc work everywhere; Connect and Continue show ↵.

"Add a mailbox" replaces the row of provider buttons: the address decides
the provider. When discovery is refused with `provider_not_configured` (IMAP
turned off on this instance), the dialog falls back to the provider buttons
the instance offers, so turning IMAP off never takes Gmail or Zoho away.

### 2. Whether IMAP is offered

| | How the screen learns it |
|---|---|
| A new field on `identity.list_ways_in` | identity knows nothing of IMAP; a cross-module coupling for a flag |
| A new `connectors` read | an endpoint for one boolean |
| **Ask discovery; `provider_not_configured` means off** (chosen) | no new surface; the dialog adapts on the first answer |

### 3. The provider guides

Chosen by `provider_hint`, else by the address's domain (a short client
table), else the generic custom-domain text. Each guide is two to four
plain steps and one link to the provider's own public help page (verified
2026-10-25):

| Guide | Ids / domains | Link (app password) | Turn on IMAP first |
|---|---|---|---|
| Yahoo | `yahoo.com`; `yahoo.*`, `ymail.com`, `rocketmail.com` | help.yahoo.com/kb/SLN15241.html | no |
| AOL | `aol.com`; `aol.*` | help.aol.com/articles/create-and-manage-app-password | no |
| iCloud | `me.com`; `icloud.com`, `me.com`, `mac.com` | support.apple.com/en-us/102654 | no |
| Fastmail | `messagingengine.com`; `fastmail.*` | www.fastmail.help/hc/en-us/articles/360058752854-App-passwords | no |
| GMX | `gmx.com`, `gmx.net`; `gmx.*` | support.gmx.com/security/2fa/application-specific-passwords.html (gmx.com), hilfe.gmx.net/sicherheit/2fa/anwendungsspezifisches-passwort.html (gmx.net/de/at/ch) | yes: support.gmx.com/pop-imap/toggle.html, hilfe.gmx.net/pop-imap/einschalten.html |
| web.de | `web.de` | hilfe.web.de/sicherheit/2fa/anwendungsspezifisches-passwort.html | yes: hilfe.web.de/pop-imap/einschalten.html |
| Yandex | `yandex.ru`; `yandex.*`, `ya.ru` | yandex.com/support/id/en/authorization/app-passwords | yes: yandex.com/support/yandex-360/customers/mail/en/mail-clients/others |
| Your own domain | anything else | none (the hosting company's help) | — |

The links are constants in the client, opened in a new tab
(`rel="noopener noreferrer"`); none is built from what the server or the
member typed. Mail.ru is in the backend's table but gets the generic text
(no guide written for it yet).

### 4. Errors

Stable codes only, each a sentence in four languages, never `title` or
`detail`: `imap_login_refused` (with "this provider needs an app password"
when discovery said `app_password`), `smtp_login_refused`,
`imap_unreachable`, `smtp_unreachable`, `imap_tls_failed`,
`smtp_tls_failed`, `imap_not_an_imap_server`, `imap_busy`, `host_refused`,
`starttls_missing`, `imap_servers_required` (opens Advanced),
`use_native_provider`, `imap_address_invalid`, `imap_username_invalid`,
`imap_password_invalid`, `too_many_attempts`, `rate_limited` (429),
`account_connected_by_another_member`, `provider_not_configured`, and a
generic one. `step_up_required` shows the screen's "Sign in again first"
banner inside the dialog, whose button is the existing `signInAgain`.

### 5. The password in the client

- `autocomplete="new-password"` on its field: no browser fills in the
  member's Campbooks password (which would then be sent to a mail server);
  `spellcheck` off. The address field is `autocomplete="email"`.
- Connect is a plain `await api.POST(…)` inside the handler, never a
  `useMutation`: React Query keeps a mutation's variables in its cache after
  it settles. The field is cleared the moment the request is made, on
  success, on cancel and when the dialog closes; nothing logs a request
  body (`lib/api` has no body logging; telemetry carries codes only).

### 6. The address check

A password login proves the username, not the address typed beside it. An
IMAP mailbox whose username differs from its address cannot send (#313)
until its address is shown to be the mailbox's.

| | How the address is shown to be the mailbox's |
|---|---|
| The member types a code we mail | friction for every custom-domain mailbox; the member types nothing is the brief |
| A code sent by the instance's own mail (to the address, through its real MX), found in the mailbox's Inbox | proves the mailbox receives the address's mail even against a hostile server; but needs instance mail on every install, and makes the instance mail arbitrary addresses on a member's word |
| **A code sent through the mailbox's own SMTP server, to the address, found in the same connection's Inbox over IMAP** (chosen) | no instance mail; defends the case #313 named (a lax but honest relay that lets any sender through delivers the code to the real mailbox of the address, not to the member's); bounded, rate-limited, every step through the existing guard, TLS and bounded framers |

What it does not defend, plainly: the member holds the mailbox's IMAP
password and can `APPEND` any message to its Inbox, so the check holds only
while the member cannot see the code. It fails (the member can verify an
address that is not theirs) at a server whose operator is the member, at a
server that files submitted mail in Sent (the member reads the code there),
and at a relay that bounces the message back to the authenticated account.
It does not stop a lax shared relay outright; it stops one that delivers
the code to the address's real mailbox and keeps no copy the member can
read. Recorded as a known limit.

**The code.** 128 bits: the first 16 bytes of the instance signer's HMAC
over `connectors.address_check` and `<connection id>:<nonce>:<address>`
(the address trimmed and lower-cased; the nonce, fresh per check and kept
with it, proves nothing alone). It travels in the check message's
`Message-ID`, `<cb-check-<32 hex>@<address domain>>`, and in no log, job
payload or event. A retry draws a new nonce: an older message never counts.

**Bound to one address.** The check row keeps the address it checks (the
connection's, when it started). The code is made from the connection's
address when the message is sent and when it is looked for; a run looks
only while that is still the checked address. Connecting a mailbox again
with its address unverified (a new address, or the same one) ends every
check under way (`failed`, `address_changed`) in the transaction that saves
it, whatever the daily budget leaves, and `verified` sets
`address_verified` only when, in its transaction, the connection's address
is still the checked one.

**Over after its window.** A check whose message was sent more than 15
minutes ago is over: the run's look ends it `failed` (`not_found`) before
anything is searched, a code found that late verifies nothing, and a check
no run looks at (a paused or failing stream) shows as failed once stale.

**The message.** From and To the address, plain text (UTF-8, base64), its
subject and body in the member's locale (the locale of the connect or
retry request, kept with the check), clearly from Campbooks, saying why it
came and that it can be ignored or deleted; `Auto-Submitted:
auto-generated` and `X-Auto-Response-Suppress: All`, so no auto-reply
answers it. Built by its own small builder (ASCII on the wire, the subject
an RFC 2047 encoded word), so no member input becomes a header.

**Sending it** (`connectors.send_address_check`, queue `sync`, a job per
connection, payload: the connection id only): the SMTP submission of #313
(`Submission::open` / `offer` / `message`) with the connection's sealed
password, the login's, workspace's and host's sending budgets of #313 (one
message, one recipient) and the login's one SMTP session slot. A 5xx to
`MAIL`, `RCPT` or the message (the server will not send as or to this
address), a refused password or an unsafe server ends the check `failed`
with a stable code; a busy server or one that cannot be reached is retried
(up to six attempts); a connection lost after the message went out counts
as sent (the search decides). Once sent, the stream is made due at once.

**Finding it** happens in the IMAP stream's own run, over its session,
after the pending changes are made and before the read (so a found and
removed message is never read in): no third session per login, the run's
slots and limits. While a check waits, the stream's next run is at most the
poll's minimum interval away. In the Inbox, opened read-only (`EXAMINE`):

1. `UID SEARCH HEADER Message-ID "cb-check-<code>"`; more than five UIDs is
   a lying answer and counts as none.
2. `UID FETCH <uids> (UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)])`; a UID
   counts only when its `Message-ID` is exactly the expected one (a
   constant-time comparison). A server that answers every search with hits
   finds nothing this way.

Found: `address_verified` becomes true in the same transaction that ends
the check `verified`. Not found: the search count goes up; 15 minutes after
the send (or 30 searches) the check ends `failed` (`not_found`).

**Removing it**, only after it was found, and only with the safe write
paths of #312, never `EXPUNGE` without UIDs or `CLOSE`:

| | How the check message goes |
|---|---|
| **UIDPLUS: `SELECT INBOX`, the UID's `Message-ID` checked again, `UID SEARCH DELETED` finds no other message marked deleted, `UID STORE <uid> +FLAGS.SILENT (\Deleted)`, `UID EXPUNGE <uid>`** (first) | gone for good, exactly that UID; an expunge answer naming more than one message is logged as hostile and ends the run; a failed expunge takes the mark off again |
| **MOVE and a Trash folder the stream knows: `UID MOVE <uid> <Trash>`** (second) | recoverable; at most one message may be expunged |
| **Neither: left in place** (third) | the member sees one message from Campbooks; nothing is risked |

The folder is left with `UNSELECT`, or `EXAMINE` of it. A server that keeps
its own copy of what it sends (Yahoo, AOL) keeps the check in Sent: it is
left there.

**Retrying** (`POST …/connections/{connection_id}/address-check`,
`connectors.check_address`, scope `connectors:write`, access Write on the
connection, Risk::Low, no body, 202 with the connection): only an IMAP
connection whose address is not verified (else 409
`address_check_not_needed`), not needing the member (409 `needs_reauth`),
whose last check ended or went stale; a new nonce, the caller's locale, the
send enqueued. Limited to 5 checks a day per connection
(`address_check:<connection>`, shared with the connect's), 429
`rate_limited`; the sending budgets apply too.

A check that dies with its job (attempts spent, a process lost) would wait
for ever: the connection view shows a check older than 30 minutes that is
still sending or waiting as `failed`, and the retry accepts it.

### 7. IMAP on by default

`CAMPBOOKS_CONNECTORS__IMAP__ENABLED` defaults to true. The startup rules:

- IMAP's own checks (`MAX_CREATED_FOLDERS`, the send settings) run only
  while it is on; their defaults pass with any valid poll bounds, so
  production (poll 60 to 120 seconds, no IMAP variable) starts. IDLE stays
  off by default and keeps its own checks (`IDLE_POLL_SECONDS` is tied to
  the poll bounds only while IDLE is on).
- IMAP needs the instance secret key (it seals the password). Before this
  change the module refused to start when IMAP was on without one; with the
  default on, that would stop every install without a key. So only an
  **explicit** `…__ENABLED=true` without a key stops the server, naming the
  variable; the default without a key logs a warning and IMAP is not
  offered.

## Data ownership and events

Schema `connectors`. Migration
`20261104010000_connectors__address_verification.sql`:

- `connectors.address_checks`: `connection_id uuid PRIMARY KEY`,
  `workspace_id uuid NOT NULL`, `FOREIGN KEY (connection_id, workspace_id)
  REFERENCES connectors.connections (id, workspace_id) ON DELETE CASCADE`,
  `address text NOT NULL` (the address checked, lower-cased), `state text CHECK IN ('sending', 'waiting', 'verified',
  'failed')`, `reason text NULL` (a stable code), `nonce uuid NOT NULL`,
  `locale text CHECK IN ('en', 'pt-PT', 'es-ES', 'fr')`, `searches integer
  NOT NULL DEFAULT 0`, `sent_at timestamptz NULL`, `updated_at timestamptz
  NOT NULL`. Row-level security enabled and forced (`workspace_isolation`),
  `SELECT, INSERT, UPDATE, DELETE` to `campbooks_app`. No code and no
  content: a state, codes, a nonce, the checked address and times.

No event is added or changed: `address_verified` is the connection's own
column, read by the send job at send time and by the connection view. The
job's payload is the connection id; its errors are fixed wording.

## Operations and scopes

- `connectors.check_address`: new, above (`connectors:write`).
- `ConnectionView` gains `address_check`: `checking` | `failed`, or null
  (verified, or never needed).
- `connectors.discover_imap` and `connectors.connect_imap`: unchanged.

## Tracked events

None. The connectors module has no Coverage rows yet (its operations are
instrumented in campbooks-cloud#332); `connectors.check_address` and
`connectors.send_address_check` join that work.

## Risks

- A hostile operator can verify an address on their own server (above).
- The check lands in the inbox before it is found if the run's read comes
  first; it then leaves at the next run (or the next sweep without
  QRESYNC).
- Yandex says a new app password can take hours to work: its guide says so,
  and a refusal names the app password.
