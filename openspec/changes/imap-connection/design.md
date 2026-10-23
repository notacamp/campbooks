# Design

## Context

Connections today come from OAuth only: `start_connection` hands off to
identity's consent door, identity runs the provider round trip and gives the
grant to `GrantIntake::adopt`, and `grants::create_or_refresh_connection`
saves the connection, seals the tokens and starts one stream per capability
and mailbox. An IMAP mailbox has no consent screen, no `sub` and no
refresh token, so it needs its own front door inside `connectors`. It ends
in the same seam, so events, streams, removal and health behave as for
Gmail and Zoho.

## Alternatives considered

### IMAP client crate

| | `async-imap` | `imap-next` / `io-imap` (Pimalaya) | `imap` 3.x |
|---|---|---|---|
| Async on tokio | yes (`runtime-tokio`) | yes (sans-I/O core) | no (blocking) |
| IDLE, CONDSTORE/QRESYNC, MOVE, SPECIAL-USE | IDLE + extensions by raw command where missing | all, typed | partial |
| Production use | Delta Chat / chatmail on thousands of servers | Himalaya | many, mostly sync |
| API churn | stable 0.x | renamed twice in 2026 (`imap-client` → `io-imap`) | 3.0 still alpha |

**Chosen: `async-imap`.** It has the widest field record against odd
servers, which is what matters most for "any mailbox". What it lacks can be
sent as raw commands behind our own small `imap::Session` wrapper, so the
crate stays swappable.

### SMTP

`lettre` is already a workspace dependency (instance mail) with
tokio + rustls. Reused; no new crate.

### Settings discovery

| Source | Kept? | Why |
|---|---|---|
| Thunderbird ISPDB fetched from `autoconfig.thunderbird.net` | no | sends every member's email domain to a third party at each connect |
| **ISPDB bundled** (MPL-2.0 XML files from `thundernest/autoconfig`, refreshed by `cargo xtask ispdb`) | yes | covers the big providers offline, no third party |
| **Domain autoconfig** (`https://autoconfig.<domain>/mail/config-v1.1.xml`, then `https://<domain>/.well-known/autoconfig/mail/config-v1.1.xml`) | yes, through the host guard, HTTPS only, no redirects, 64 KiB cap | how hosting companies publish settings |
| **DNS SRV** `_imaps._tcp`, `_imap._tcp`, `_submissions._tcp`, `_submission._tcp` (RFC 6186/8314) | yes | standard; cheap |
| **MX host** → ISPDB lookup by MX domain, then `imap.<domain>` / `smtp.<domain>` guesses | yes, guesses must still pass verify | covers custom domains on big providers (an MX at `mx01.mail.icloud.com` → iCloud) |
| Microsoft Autodiscover | no | Microsoft goes through Graph (#309) |

The MX step also **routes**: MX at Google (`*.google.com`,
`*.googlemail.com`), Zoho (`*.zoho.*`, `*.zohomail.*`) or Microsoft
(`*.mail.protection.outlook.com`, plus the outlook.com / hotmail / live / msn
domains) answers `use_provider: google | zoho | microsoft` instead of IMAP
settings, because those are connected natively with OAuth (Microsoft once
#309 lands; until then it answers `not_supported_yet`). Gmail through IMAP
with an app password is refused for the same reason.

### Where the password lives

A `password` column on `connectors.credentials` (nullable;
`refresh_token` becomes nullable too; a CHECK requires exactly one of the
two). Putting the password in `refresh_token` was rejected: every
token-refresh path would have to know it is not a token.

## Data ownership and events

Schema `connectors` only. Migration
`20261023130000_connectors__imap.sql`:

- `connections.provider` CHECK gains `'imap'`.
- `credentials.refresh_token` DROP NOT NULL; `credentials.password bytea`;
  `CHECK ((refresh_token IS NULL) <> (password IS NULL))`.
- New `connectors.imap_servers` (one row per IMAP connection, RLS by
  workspace like `credentials`): `connection_id PK FK ON DELETE CASCADE`,
  `workspace_id`, `username text`, `imap_host`, `imap_port`,
  `imap_security CHECK IN ('tls','starttls')`, `smtp_host`, `smtp_port`,
  `smtp_security CHECK IN ('tls','starttls')`, `source CHECK IN
  ('ispdb','autoconfig','srv','mx','manual')`, `provider_hint text NULL`
  (the ISPDB id, e.g. `yahoo.com`, for app-password guides), `updated_at`.
- Identity of the account: `account_subject = '<lower(username)>@<lower(imap_host)>'`.
  `address` is the email address the member typed (lower-cased);
  `address_verified` is true only when the username equals the address
  (a successful login proves the username, not the address).
- Capabilities: `messages` only.
- Events: unchanged, `ConnectionAdded` / `CapabilityGranted` through
  `create_or_refresh_connection`. `provider` in `connection-account.v1`,
  `connectors.connection_removed.v1` and the events catalogue gains `imap`.

The password is sealed with the existing connectors key, AAD
`connectors.credentials:password:<connection id>`. `credentials::password(conn,
cipher, id) -> Result<SecretString, TokenError>` is the only reader;
`refresh_at_provider` and `removal`'s revoke treat `imap` as "nothing to
refresh / revoke".

## Operations

Both under `/api/v1/w/{workspace_id}/connections/imap`, bearer scope
`connectors:write`, member tokens only (`connection_needs_user` for client
tokens), access `connectors.connection` Create (ADR 0017), **step-up**: a
sign-in within 10 minutes (`step_up_required`, same as the consent door).
The rule moves from `identity::domain::step_up` to
`campbooks_kernel::auth::step_up` so both modules call one implementation.

- `POST …/imap/discover` → `connectors.discover_imap`, `query()` (no
  write), rate limit 30/hour per member. Body `{ address }`. Answer one of:
  - `{ kind: "settings", imap: {host, port, security}, smtp: {…}, username_hint, source, provider_hint, password: "app_password" | "account_password" | "unknown" }`
  - `{ kind: "use_provider", provider: "google" | "zoho" | "microsoft" }`
  - `{ kind: "not_found" }` (the client then asks for the servers by hand)
  `password` comes from a small curated table (Yahoo, AOL, iCloud, GMX,
  web.de, Yandex, Mail.ru, Fastmail: app password) used by the guides in
  #314; it never changes behaviour.
- `POST …/imap` → `connectors.connect_imap`, `command(Risk::Low)`,
  `.secret_once()`, `.records(["connectors.connection"])`, rate limits
  below. Body `{ address, username?, password, servers? , connection? }`;
  `servers` overrides discovery (`source: manual`); `connection` names an
  existing IMAP connection to reconnect (new password or servers; same
  `account_mismatch` / someone-else's-connection rules as OAuth reconnect).
  Verifies, then calls `create_or_refresh_connection`. Answers the
  connection (never the password).

Both answer `provider_not_configured` while
`CAMPBOOKS_CONNECTORS__IMAP__ENABLED` is false. `discover_imap` answering
`use_provider` is a 200, not an error.

### Verify

1. Resolve each host through the guard (below); connect to the checked
   address with a 10 s connect timeout, 30 s overall.
2. IMAP: TLS (993) or STARTTLS (143; refused if the server does not offer
   STARTTLS). `CAPABILITY`; refuse if `LOGINDISABLED` persists after TLS.
   `AUTHENTICATE PLAIN` when offered, else `LOGIN`. `LIST "" "INBOX"` must
   answer. `LOGOUT`.
3. SMTP: TLS (465) or STARTTLS (587) via `lettre`'s `test_connection` plus
   AUTH (no `MAIL FROM`). An SMTP server that offers no AUTH is refused.
4. Refusals (stable codes, en/pt-PT/es-ES/fr messages):
   `imap_login_refused` (with `app_password_needed: true` when the
   provider table says so), `smtp_login_refused`, `imap_unreachable`,
   `smtp_unreachable`, `imap_tls_failed`, `smtp_tls_failed`,
   `imap_not_an_imap_server`, `host_refused` (the guard),
   `starttls_missing`. Server greeting and response text is **never**
   echoed (it can carry the username or internal host names); only the
   IMAP response code (`[AUTHENTICATIONFAILED]`, `[UNAVAILABLE]`…) is
   logged.

## The host guard

`identity/src/metadata/guard.rs` moves to `campbooks_kernel::net::guard`
(identity re-exports it; behaviour unchanged). It gains
`resolve_socket(host, port) -> Result<SocketAddr, Refused>` that resolves,
refuses unless **every** answer is public, and returns the checked address
to connect to (no second lookup, so DNS rebinding can't swap it). Used for
the IMAP host, the SMTP host, and the autoconfig HTTPS fetch, at verify
and (in #311+) at every session. TLS still verifies the certificate
against the host **name**.

`CAMPBOOKS_CONNECTORS__IMAP__ALLOW_PRIVATE_HOSTS` (default false) lets a
self-hoster reach a mail server on their own network. Ports are limited to
993, 143, 465, 587 unless `…__ALLOW_ANY_PORT` is set.

## Abuse limits

Connect attempts make Campbooks log in to someone else's server, so the
endpoint must not become a password-guessing service. Through the existing
`schedule::take` budgets:

- per member: 10 connect attempts / hour;
- per workspace: 30 / hour;
- per target mailbox (hash of `<lower(username)>@<lower(imap_host)>`, the
  login actually tried, so typing another address does not reset it; across
  all workspaces): 5 **failed** logins / hour, then `too_many_attempts` for
  an hour.

Discovery: 30 / hour per member.

## Settings

`CAMPBOOKS_CONNECTORS__IMAP__*`: `ENABLED` (false), `ALLOW_PRIVATE_HOSTS`
(false), `ALLOW_ANY_PORT` (false), `CONNECT_TIMEOUT_SECONDS` (10),
`TIMEOUT_SECONDS` (30). `imap` is "configured" when `ENABLED` and the
cipher is present.

## Sync while this slice stands alone

`create_or_refresh_connection` starts the `messages` stream as for any
provider. Until #311, `sync_stream`'s `imap` arm returns no page and
leaves the stream idle at the slowest interval; `send_message`'s arm
answers `provider_not_configured`. Both arms exist so every `match` on
`Provider` stays exhaustive.

## Risks

- **Credential in our hands.** A mailbox password is worth more than an
  OAuth token (no scope, no revocation at the provider). Mitigations: app
  passwords recommended everywhere, sealed at rest, never returned, never
  logged, deleted on disconnect; documented in the self-hosting security
  notes.
- **Odd servers.** Discovery can find wrong settings; verify catches it
  and the manual override covers the rest.
- **Bundled ISPDB goes stale.** `cargo xtask ispdb` refreshes it; domain
  autoconfig and SRV still apply.
