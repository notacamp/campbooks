# Design

## Context

`connectors` owns connections, credentials, the sync engine (streams,
cursors, adaptive polling, the per-provider budget, health) and the Gmail
adapter, which reads one bounded page per run and writes back members'
changes and sends (ADR 0010, #460, #461). `identity` is the one front door
for provider consent (ADR 0014): it runs the round trip, validates Zoho's
data centre (`accounts-server`, a fixed list), and hands a mailbox grant to
`connectors` through the kernel's grant-intake port, which creates or
refreshes the connection through one seam (`create_or_refresh_connection`).

Zoho differs from Gmail in ways that shape the design:

- **Data centres.** A Zoho account lives in one data centre; its tokens are
  refreshed at that centre's accounts server and its mail is served by that
  centre's mail host (`accounts.zoho.eu` → `mail.zoho.eu`).
- **No push, no history.** Zoho Mail's REST API lists a folder's messages
  newest first (`messages/view`, offset paging, at most 200) and has no
  change log, no `modifiedTime` filter and no notification channel.
- **Folders, a flag and an archive bit** instead of labels: a message is in
  one folder, read or unread, flagged (`info`, `important`, `followup`) or
  not, and archived or not (an archived message is hidden from listings).
- **One login, several mail accounts** (`GET /api/accounts` lists them).
- **Metadata is HTML-escaped** (`&lt;`, `&quot;`, `&#39;`), and it carries
  no bulk-signal headers; the original MIME message does.
- **Send takes JSON** (`fromAddress`, `toAddress`, `content`, …) or a reply
  to a Zoho message id; the caller cannot set `Message-ID`.

## Alternatives considered

### API: Zoho Mail REST vs IMAP + SMTP with OAuth

| | REST (chosen) | IMAP + SMTP (XOAUTH2) |
| --- | --- | --- |
| Consent | the same Zoho app and round trip as sign-in; `ZohoMail.*` scopes | Zoho documents XOAUTH2 for IMAP and POP only (`ZOHOMAIL.PROTOCOL.IMAP`), registered as a *Self Client*; SMTP with OAuth is undocumented |
| Machinery | `reqwest`, as Gmail; one short request per call | a new IMAP client crate plus TLS, a stateful connection per mailbox, an SMTP client |
| Fit with the engine | one bounded page per job, no long-lived state | IDLE wants a held connection per mailbox: a new long-running role |
| Change detection | list newest page per folder + periodic sweep | `UIDVALIDITY`/`UIDNEXT`/`CONDSTORE` are better than REST listing |
| Send | JSON API; no control of `Message-ID` | raw RFC 5322 with our `Message-ID` |
| Self-host weight | none | a persistent connection per mailbox |

IMAP would give better change detection and our own `Message-ID`, but Zoho
does not document OAuth for SMTP, IMAP needs a Self-Client registration
separate from the sign-in app, and IDLE does not fit job-shaped runs. The
Rails app on `main` also used the REST API; that choice is kept on these
merits, not copied: none of its code or schema is reused, and its fix for
HTML-escaped metadata becomes a decoding step where metadata is read.

### Change detection: head + sweep vs full listing every run

Listing every folder's whole window each run would see every change but
costs pages per folder per minute for a large mailbox. Reading only the
newest page per folder each run (a handful of small requests), plus a sweep
of the window every few hours, sees new mail and changes to recent mail at
once, and older changes within hours. Chosen: head + sweep.

### Several mail accounts: one stream each vs one connection each

A connection is a member's grant (one consent, one token). Splitting the
mail accounts of one Zoho login into several connections would duplicate
the token and its health. Chosen: one connection, one `messages` stream (and
inbox) per mail account, keyed by Zoho's `accountId`.

### Where the data centre comes from

The ID token's issuer, which identity already requires to be one of Zoho's
accounts servers (or, in development, the fake). Alternatives rejected:
Zoho's `api_domain` in the token answer (a different host family, and an
input we would have to validate again) and probing data centres.

## Data ownership and events

All in schema `connectors` (migration `connectors__zoho_mail`):

- `connections.provider` accepts `zoho`; `connections.provider_server` holds
  the accounts server a Zoho grant came from (NULL for Google).
- `streams.mailbox_key` (Zoho's `accountId`; empty for Gmail) and
  `streams.address`; a connection has one stream per capability **and
  mailbox** (`UNIQUE (connection_id, capability, mailbox_key)`).
- `gmail_messages` → **`messages`**, `gmail_oversized` → **`oversized`**:
  the adapters' index, plus `rfc_message_id` (a reply is sent as a reply to
  the Zoho message with that id). Zoho's state is stored as label-like
  tokens so the outbound-change machinery stays one: `folder:<id>`,
  `INBOX`, `SENT`, `SPAM`, `TRASH`, `UNREAD`, `STARRED`, `ARCHIVED`.
- `provider_budgets` keys become free text (`google`, `zoho:<connection>`).

Events are unchanged: `connectors.capability_granted` is published once per
new **mailbox stream** (one inbox each, with that mail account's address in
its connection-account document, whose `provider` gains `zoho`);
`messages_changed`, `send_completed` and the consumed
`conversations.changes_requested` / `send_requested` are as for Gmail.
Operations and scopes are unchanged (`connectors:read`, `connectors:write`);
`ConnectionView.streams[]` gains `address`.

The kernel's `ProviderGrant` gains `issuer` (the ID token's `iss`): identity
sets it on a connect/reconnect and on a held grant (from the provider
identity's stored issuer). `connectors` takes a Zoho grant only when its
issuer is one of Zoho's accounts servers, or the configured fake in
development (`unknown_data_centre` otherwise).

## Consent and scopes

`mailbox_scopes("zoho")` is `ZohoMail.accounts.READ ZohoMail.folders.ALL
ZohoMail.messages.ALL`, so the sign-in, connect and reconnect round trips
ask for them with `access_type=offline` (and `prompt=consent` on the
existing retry when no refresh token came back). The Zoho OAuth app must
allow exactly: `openid`, `email`, `profile`, `ZohoMail.accounts.READ`,
`ZohoMail.folders.ALL`, `ZohoMail.messages.ALL`. Zoho documents scopes as
comma-separated and its token answer may omit `scope`; granted scopes are
read split on spaces and commas, and a Zoho answer without `scope` counts
as granting what was asked (Zoho's consent screen is all or nothing).

## The adapter

Endpoints for a connection with accounts server `https://accounts.zoho.X`:
token `…/oauth/v2/token`, revoke `…/oauth/v2/token/revoke`, mail
`https://mail.zoho.X/api/` with `Authorization: Zoho-oauthtoken <token>`.
With `CAMPBOOKS_ZOHO__BASE_URL` (development only) every one is under that
base. Calls used:

| Purpose | Call |
| --- | --- |
| mail accounts of the login | `GET accounts` |
| folders | `GET accounts/{a}/folders` |
| newest messages of a folder | `GET accounts/{a}/messages/view?folderId=&start=&limit=&sortorder=false&includeto=true` |
| raw message | `GET accounts/{a}/messages/{m}/originalmessage` (`data.content`) |
| where a vanished message went | `GET accounts/{a}/folders/{f}/messages/{m}/details` |
| writes | `PUT accounts/{a}/updatemessage` modes `markAsRead`, `markAsUnread`, `setFlag`, `archiveMails`, `unArchiveMails`, `moveMessage` |
| send / reply | `POST accounts/{a}/messages`, `POST accounts/{a}/messages/{parent}` with `action: reply`; attachments first with `POST accounts/{a}/messages/attachments?fileName=` |

Ids are 64-bit and arrive as numbers or strings; they are kept as strings
and sent back as numbers.

### Reading (cursor per folder)

The cursor holds the tracked folders (Inbox, Sent, Spam, Trash and user
folders; never Drafts, Templates, Outbox or Snoozed), each with its walk
position, plus the window start, a generation and when the last sweep ended.

- **Backfill / sweep (`backfill`, `resync`)**: walk each folder newest first,
  200 per listing, down to the window start (default 90 days); new messages
  are downloaded as raw MIME (at most 50 per run and the run's byte budget,
  as Gmail), known ones compared. At the end of a sweep, indexed messages of
  the window that no listing showed are resolved (below).
- **Incremental**: the newest 50 of every tracked folder. New ones are
  downloaded; known ones whose state tokens differ are re-emitted from the
  stored document, without a download. A known message that has left the
  range its folder's page covered, and appears in no other listing, is
  looked up with `details`: still in its folder → archived; in another
  folder → moved; gone → removed.
- A sweep starts every `CAMPBOOKS_CONNECTORS__ZOHO__SWEEP_MINUTES` (360).

State: `in_inbox` = Inbox folder and not archived; `in_spam`/`in_trash` by
folder; read from `status`; starred = any flag; outbound when in Sent. No
categories, no labels, `thread_key` none (Zoho only gives a thread id once a
thread has two messages, so conversations threads by `In-Reply-To` /
`References`, as it already does).

### Write-back

The merge, supersede, trash and give-up rules of #460 are shared, moved
from `gmail/write.rs` into `domain/outbound`. Zoho has no history, so
changes carry no stamp and apply in arrival order. Intents map to
`UNREAD` (`markAsRead`/`markAsUnread`), `STARRED` (`setFlag important` /
`flag_not_set`) and `INBOX` (archive: `archiveMails`; unarchive:
`unArchiveMails` for an archived message, `moveMessage` to the Inbox folder
for one elsewhere). Messages with the same change go in one call (the API
takes id lists); a refused group is retried one message at a time. After a
write, the index holds the state written, so the next run emits nothing.

### Send

The JSON is built from the outgoing document (From the mailbox, To/Cc/Bcc,
subject, the HTML alternative or the text, uploaded attachments). A reply
whose `In-Reply-To` is an indexed message of the stream goes through
Zoho's reply call, so Zoho sets `In-Reply-To`/`References` at the
recipients and conversations threads the sent copy. #461's rules hold: an
attempt is committed before the call; while a send is unclear, the next
attempt first looks in the Sent folder for a message sent since that
attempt with the same subject and recipients and no other send's id; no
call comes sooner than the timeout plus 30 seconds after the last. When
Zoho accepts, the sent message is fetched as raw MIME and indexed and
announced in the job's transaction; if that fetch fails the send is still
`sent`, and the next sync brings the copy from the Sent folder.

### Quotas and rate limits

Each Zoho login has its own request budget,
`CAMPBOOKS_CONNECTORS__ZOHO__REQUESTS_PER_MINUTE` (default 60, a minute's
worth of burst), in the shared `provider_budgets` table, so one mailbox's
backfill never spends another's. A run takes its worst case (one listing
per folder, its downloads and look-ups) before it starts. Zoho's 429 (and
errors coded as too many requests) throttle that stream until
`Retry-After`, as for Gmail; 401 drops the access token and refreshes, and
a refresh answered with `invalid_code`/`invalid_grant` (Zoho may answer 200)
sets `needs_reauth`.

## Settings

`CAMPBOOKS_ZOHO__CLIENT_ID`, `…__CLIENT_SECRET(_FILE)` (shared with sign-in,
one OAuth app), `…__BASE_URL` (development only),
`CAMPBOOKS_CONNECTORS__ZOHO__{BACKFILL_DAYS,MAX_MESSAGE_BYTES,
RUN_BUDGET_BYTES,REQUESTS_PER_MINUTE,SWEEP_MINUTES}`.

## Risks

- Zoho's API is verified against its documentation and the fake, not a
  live mailbox (the owner's rule). An ignored test runs against a real
  account with a refresh token, as for Google.
- A message moved within Zoho to a folder past that folder's newest page is
  first seen as moved by its `details` look-up, or else removed and
  re-added by the next sweep.
- The Sent-folder look-up could take an identical message the member sent
  from another client within the same seconds for ours: the send is then
  recorded `sent` without a second copy going out, the safe side.
