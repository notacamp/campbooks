## Context
Batch 3 of the identity security audit (campbooks-cloud#117). Batch 1
(`fix/117-access-guards`) builds the kernel authorization catalogue; this
change adds no parallel mechanism: it keeps `Unbound<S>` and the existing
404 rules, and only tightens what those already decide.

## Decisions

### Way-in notices by mail, from a kernel mailer
`kernel::mail` holds one `Mailer` port in `AppState`. The server builds it
from `CAMPBOOKS_MAIL__SMTP_URL` (or `_FILE`) and `CAMPBOOKS_MAIL__FROM`: both
set, an SMTP transport (lettre, rustls with the platform verifier, aws-lc-rs:
already the TLS stack of the graph); neither, `NoMailer`; one without the
other, a startup error naming the setting. The URL carries the password: it
is a `SecretString`, never logged, and errors never repeat it; a send error
is reduced to `Refused` / `Unavailable` / `BadRecipient` so no server reply
(which can echo an address) reaches a log or a job's `last_error`.

`identity.notify_way_in_added` consumes `identity.way_in_added` (which now
names the way in it added). A sign-up's first way in gets no notice. The
recipients are the account's primary address and the new provider way in's
address, each only when verified (an unverified address may be anyone's; a
notice to it would tell a stranger about the account), deduplicated. The
message is plain text and HTML saying the same, in the language the person
chose (`LocalePreferencePort`, English otherwise), naming what was added and
when, with one link: the instance's public URL. No images, no remote content,
no tracking. Idempotency: after each successful send a row
`identity.notices(event_id, recipient)` is committed on its own, so a
delivery retried after a later failure skips what already went; the row holds
which of the account's addresses it was (`account` / `way_in`), never the
address. An unreachable server retries the delivery (backoff); a permanent
refusal is logged without the address and not retried. Every delivery leaves
an audit row (`way_in_notice`: sent, refused, or `no_mail_transport`) with
ids and counts only.

Without a mail server the notice is the audit row plus the app: Ways to sign
in marks a way in (provider or passkey) added in the last seven days "New"
and says what to do if it wasn't you (remove it, then sign out everywhere).
Invitations will reuse the mailer later; not in this change.

### Sign out everywhere
`POST /api/v1/identity/sign-out-everywhere` (`identity:write`, `Unbound`)
ends every live issuer session of the account and revokes every refresh
family (all clients: the web app, the native app, third-party apps), in one
transaction with an audit row and `identity.signed_out_everywhere {account}`.
The BFF consumes it (`bff.end_signed_out_sessions`) and deletes its sessions
of that subject. No recent sign-in is asked: whoever fears a stolen session
must be able to end it at once, and the worst a thief can do with it is sign
the owner out. Access tokens already issued stop at their expiry (minutes).
The web app then signs its own browser out through `/bff/logout`.

### A copied web session ends
`find_session` now finds a session by its current or previous token and says
which; `Inner::current` treats a previous token presented after
`ROTATION_GRACE` (two minutes) as a replay: the cookie was copied, and either
copy may be the thief's. The session ends everywhere it reaches (row deleted,
refresh token revoked at the issuer, issuer session ended with the ID token),
the same teardown `/bff/logout` uses (now shared as `Inner::terminate`), and a
warning is logged without ids. Older tokens are kept too: each rotation moves
the token before the previous one into `bff.retired_tokens` (its SHA-256),
kept until the session ends (cascade). A retired token presented at any time
ends the session the same way, so an idle victim's copied cookie still trips
it while the thief keeps the session alive. The memory is bounded: a row per
rotation (an hour by default), or per reissue (below), which waits out the
two-minute grace, so at most about one row per two minutes of a session's
life, usually about 720 over the default 30 days. A rotated-away token is a theft
signal only once its successor has been presented (`token_delivered`): until
then the response carrying the new cookie may have been lost (an aborted
fetch, a reload, a sleeping laptop), and the old token gets a fresh rotation
instead of a sign-out; the undelivered successor is retired, so a copy of it
presented later still ends the session. Only one request reissues: the locked
row must still be past its grace, and the reissue starts a new one, so the
other requests a waking page fires with the same old token go through as
within the grace, with no cookie of their own; one whose successor was
presented between its lookup and the lock is a copy, and ends the session.

### Passkeys need a fresh sign-in
`start_passkey_registration` asks for a sign-in from the last five minutes
(`fresh_sign_in_required`); `register_passkey` keeps the ten-minute step-up,
since the challenge it spends was issued under the five-minute rule and lives
five minutes. The web app answers the code by signing in anew
(`/bff/login?max_age=0`).

### Authorization requests belong to their browser
`/oauth2/authorize` sets `__Host-campbooks_browser` (random, `SameSite=Lax`
since requests arrive as cross-site navigations, a day long, renewed) and
stores its SHA-256 with every request it stores. Loading or taking a request
(sign-in screen, passkey ceremonies, completion after a provider round trip
or a passkey, consent GET and POST) needs the same cookie: in another browser
the id names nothing (404, or completion falls back to the plain return
path). Requests waiting at upgrade (at most 15 minutes old) are dropped by the
migration: those sign-ins start again from the app.

### Refresh never outlives its sign-in
A new refresh family's absolute expiry is the earlier of the client's
lifetime and its issuer session's `expires_at` (`least(...)` in the insert).

### Rate limits
`/oauth2/authorize` 60 per address a minute; `/oauth2/end_session` 30 (each
verifies a signature); `/bff/login` 30 (the BFF resolves the address the way
the kernel's `ClientIp` does, with `CAMPBOOKS_TRUSTED_PROXIES`); passkey-only
sign-ups 5 per address an hour, checked before the instance's ceiling (now
200 an hour) so one address over its budget does not spend everyone's.
In-process calls (the BFF's own) have no address and are never counted.

### Members' addresses
`workspaces.users.email_verified` records what the identity provider said at
the last sign-in; members' addresses are listed only when verified, and the
"already a member" check of an invitation matches verified addresses only (an
unverified claim cannot block an invitation).

### Look-alike app names
The name is decomposed for compatibility (NFKD), stripped of every mark
(general category M, `icu_properties`), then reduced to its UTS #39
confusables skeleton (`unicode-security`, Unicode 16 data: Cyrillic, Greek,
Armenian, Cherokee, Coptic, IPA, digits of other scripts; `rn` and `m` share
one), before and after lower-casing, with leetspeak read as letters (`4` and
`@` for `a`, `$` for `s`, `()` for `o`, `æ` for `a`). The checks read the
whole normalised name: normalisation can lengthen a name many times (`ﷺ` is
18 characters), and only what is stored is cut to 100. `campbook` covers the
plural.
It reads as Campbooks when its skeleton contains the skeleton of `campbooks`
or `notacamp`, ignoring spaces and punctuation. A letter or digit nothing maps
stands for any character, and so does a symbol (or the symbol is ignored,
whichever matches), at most two per match, so `Campb○○ks` is refused while a
Japanese or Arabic name is not. Separately, a word may not mix scripts beyond
UTS #39's "highly restrictive" level (Latin with Cyrillic, Greek, Armenian,
Cherokee…, or a letter no identifier may use, such as small capitals):
`client_name_mixed_scripts` for a workspace's app; a self-registered agent is
shown under its redirect host instead. A hand-written fold table was the
earlier approach: it cannot keep up with the confusables data.

### Delta-review fixes
App names are decomposed with NFKD and stripped of combining marks before
the "reads as Campbooks / Not A Camp" folding (full-width, mathematical,
circled and accented look-alikes); client credentials re-filter a client's
stored scopes by the third-party ceiling when issuing; `Admin<P>` answers 404
to a bound principal; the identity features' account resolution answers 404
to a bound principal (so a workspace's app routes and held-grant adoption
refuse an app let into that very workspace); the admin step-up window is one
hour.

## Alternatives considered

**Notices.**
1. *Kernel SMTP mailer, notices from an event consumer (chosen).* One port,
   standard SMTP any self-hoster has, retries and idempotency from the job
   system, nothing sent inline in a request.
2. *A provider's HTTP API (SES, Postmark, Mailgun).* Simpler deliverability,
   but a vendor dependency for self-hosters and one more secret format; SMTP
   reaches all of them anyway.
3. *Send through the person's own connected mailbox.* No operator setup, but
   the account may have none, a thief who connected a mailbox could intercept
   it, and it mixes system mail with the person's.
4. *In-app only.* No setup, but the person must open the app to learn about
   a takeover, which is when they no longer can. Kept as the fallback.

**Copied sessions.**
1. *End the session on any rotated-away token presented after the grace,
   keeping retired hashes until the session ends (chosen):* no false positives
   inside the grace window; bounded memory (about one hash per two minutes
   of a session's life at most, typically one an hour).
2. *Only the last rotated-away token:* cheaper, but a thief who keeps the
   session busy while the victim is idle rotates the victim's copy out of
   reach.
3. *Bind sessions to the client IP or user agent:* breaks on mobile networks
   and proxies; user agents are forgeable.

**Fresh sign-in for passkeys.**
1. *Shorter `auth_time` window plus `max_age=0` from the app (chosen).*
2. *Check the issuer session cookie on registration:* `/bff/api/*` strips
   cookies before the API sees the request, so the API cannot read it.
3. *Re-authenticate inside the WebAuthn ceremony (an existing passkey's
   assertion):* stronger, but passkey-less accounts would need a provider
   round trip anyway; left for later.

**Browser binding.**
1. *A browser cookie hashed into the request (chosen):* the pattern provider
   round trips already use.
2. *Bind to the issuer session:* there is none before sign-in, which is when
   the id travels.
3. *Signed request ids carrying the browser:* same effect, but the cookie is
   still needed and the stored hash is simpler to reason about.

**Sign out everywhere.**
1. *Revoke at the issuer, BFF follows by event (chosen):* the module boundary
   stays (identity does not write `bff.*`); the delay is seconds, and the BFF
   session cannot refresh in the meantime.
2. *Identity deletes BFF rows directly:* immediate, but crosses schemas.
3. *A token deny-list checked per request:* ends access tokens at once too,
   at the cost of a lookup on every API call; access tokens live minutes.

**Members' unverified addresses.**
1. *Record verification and hide unverified addresses (chosen).*
2. *Store only verified addresses:* loses the address the person themselves
   typed at sign-up for later verification.
3. *Show them with an "unverified" mark:* still lets someone appear as
   `ceo@company.example` to the rest of the workspace.

Does this copy a pattern from the old Rails app on `main`? No.
