# Design

## Context

A send is asked for by the conversations module (`conversations.send_requested`
v1), recorded once by `connectors.queue_send` in `connectors.sends` (keyed by
its send id, holding the outgoing-message v1 document) and made by
`connectors.send_message` (queue `sync`, ordered by `send:<id>`, eight
attempts, a give-up check 30 minutes on). Before any call the job asks the
conversations module whether the send still stands and is still bound to
the stream, connection and mailbox it goes through (#226, #242). The Gmail
and Zoho arms make their own idempotency: an attempt is committed on its own
(`unclear = true`) before the call, cleared by a definite answer, and an
unclear send is looked up before anything is sent again. Once the provider
accepts, the job indexes the sent message and announces it like a synced one,
marks the send `sent`, publishes `connectors.send_completed` and makes the
stream due at once; the next sync recognises the message and emits nothing.

The document's `Message-ID` is minted per send by the conversations module
(`<send id>@<inbox domain>`) and kept with the draft and in the send's
document; it is what this change keys the sent message by.

An IMAP connection (#310) has its servers in `connectors.imap_servers`, its
password sealed in `connectors.credentials`, one `messages` stream whose run
signs in once, makes the pending write-backs (#312), then reads (#311),
recording every copy of a message as a location (`connectors.imap_locations`).
#310 checked the SMTP server at connect (`imap/smtp.rs`, `lettre`), never
sending. Today the IMAP arm of `send_message` fails every send with
`not_connected`.

## Decisions

### 1. The SMTP client: `lettre` over a connection Campbooks opens

| | SMTP client |
|---|---|
| `lettre`'s `AsyncSmtpTransport` | resolves the host itself (no guard, DNS rebinding), pools connections, hides which step failed, so "was `DATA` sent?" cannot be answered |
| Campbooks' own SMTP client (as #311 did for IMAP) | full control, but a second protocol implementation to keep right (dot-stuffing, multi-line replies, AUTH challenges) |
| **`lettre`'s `AsyncSmtpConnection` over a stream Campbooks connects and bounds** (chosen) | the guard's address, our TLS, the steps one by one (`MAIL`, `RCPT`, `DATA`, the message), its reply parser and dot-stuffing reused; the gaps closed around it |

`lettre` is already the connect check's client (#310). What it does not do,
the wrapper does:

- **The address.** The SMTP host is resolved through the kernel's guard at
  every send (`resolve_socket_with`, the mail ports, private addresses only
  where the instance allows them), and the TCP connection is opened to that
  checked address only (`connect_with_transport`), never to the name again.
- **TLS.** Port 465 (`tls`): TLS from the first byte with the platform
  verifier (`imap::tls::client_config`), the certificate checked against the
  host name, before the greeting is read. Port 587 (`starttls`): the server
  must offer `STARTTLS` in its first EHLO; if it does not, the session is
  aborted and nothing else is said: no `AUTH`, no password, no `MAIL`
  (`starttls_missing`). After the upgrade (`lettre`'s rustls with the
  platform verifier, as #310's check) EHLO is asked again.
- **Bounded reads.** `lettre` reads a reply a line at a time and checks a
  line's length (1,000 bytes) and a reply's (100,000) only after reading it.
  The stream it reads is wrapped: at most 1 MiB comes from the server in the
  whole session (SMTP replies are a few lines), past which every read fails
  and the session ends, so a server that never sends a line ending cannot
  grow memory.
- **Time.** Every step (resolving through the guard, the TCP connect, TLS,
  greeting and EHLO, STARTTLS, AUTH, `MAIL`, each `RCPT`, `DATA`) has
  `CAMPBOOKS_CONNECTORS__IMAP__TIMEOUT_SECONDS` (30, the connect
  `…__CONNECT_TIMEOUT_SECONDS`) and never runs past the whole submission's
  deadline, `…__SEND_TIMEOUT_SECONDS` (300, at most 600 so a submission always
  ends within its login's 15-minute session slot); the message itself has
  what is left of that deadline. `QUIT` waits 5 seconds at most.
- **Words.** `lettre`'s errors carry the server's text; none is logged or
  stored. Logs carry the reply code (`535`, `451`) and stable codes only.
- **No pipelining, no `CHUNKING`, no `BDAT`**: one command, one reply.

### 2. Signing in

`AUTH PLAIN`, else `AUTH LOGIN` (the mechanisms the connect check accepts),
with the sealed password, opened in the send's own short transaction and
dropped when the session ends. A server that offers neither is a refused
login. Never `AUTH` before TLS.

| SMTP answer to `AUTH` | What it means for the send |
|---|---|
| 235 | signed in |
| 5xx (535, 534…) | a refused password: counted with the stream's shared refusals (`health::count_login_refusal`, the same count as the runs' and IDLE's); the third in a row sets `needs_reauth` and the send fails `needs_reauth`; before that the send is retried later |
| 4xx (454…) | busy: retried with backoff, never counted |
| anything else, or no answer | unreachable: retried |

A successful IMAP run resets the count, so an SMTP password that is wrong
while the IMAP one works does not end in `needs_reauth`: the send retries
until its attempts run out and ends `unavailable` (nothing was sent).

### 3. Who it is from

| | Allowed `From` |
|---|---|
| Whatever the document says | an IMAP server that does not check the sender (many small ones) would send as anyone |
| The document's, when the server accepts it | the same, decided by the server |
| **The connection's own address only, once verified** (chosen) | the envelope sender is the connection's address as stored (its case kept; only the comparison ignores case), never the document's; a document whose `From` address is not it is not sent: `rejected`, before connecting |

There is no alias model in the send path today (Gmail sends as the
account, Zoho's `fromAddress` is checked by Zoho), so the connection's
address is the only one. That address is the one the member typed at
connect, and a password login proves the username, not an address typed
beside it: a member could connect `billing@victim-corp.example` with the
login of a mailbox on a lax server that lets any sender through. So an IMAP
send is made only while the connection's `address_verified` is true (#310
sets it when the username is the address); otherwise it fails `rejected`,
logged with the stable code `address_not_verified`, before any SMTP session
(the send's reasons are a closed set in `connectors.send_completed` v1, so
the code is not a new reason). Verifying the address of a mailbox whose
username differs from it ships with the connect screen (#314): Campbooks
sends a one-time code to the address through the mailbox's own SMTP server
and finds it in that mailbox's Inbox over IMAP. It is not built here.

### 4. Recipients and Bcc

The envelope recipients are the document's To, Cc and Bcc, each parsed as an
address (`lettre::Address`: a line break, a space or a missing `@` refuses
the whole send, `rejected`), lower-cased and de-duplicated, at most
`…__SEND_RECIPIENTS_PER_MESSAGE` (50, else `rejected`). The message on the
wire is built by the outgoing-document MIME builder (`gmail::compose`, whose
`clean` turns every control character of a header value into a space) from
the document **without** its Bcc list: no `Bcc` header goes out. The Sent
copy is built from the whole document and keeps `Bcc`, as other mail apps
keep it. Any recipient refused (`RCPT` 5xx) aborts the whole send before
`DATA` (`rejected`): Campbooks never delivers to part of a message's
recipients without saying so.

### 5. Never twice

SMTP has no idempotency key and no answer id. What a failure means depends
on where it happened:

| Where it failed | Was it sent? | What happens |
|---|---|---|
| connecting, TLS, greeting, EHLO, AUTH, `MAIL`, `RCPT`, the `DATA` command | no | 4xx or no answer: retried with backoff, the budget given back; 5xx: `rejected`, the budget kept (a refused recipient costs what it cost, so trying addresses at a server is never free) |
| a 4xx or 5xx answer to the message (after the final dot) | no (RFC 5321: the server took no responsibility) | 4xx: retried; 5xx: `rejected` (552: `too_large`) |
| a 250 answer to the message | yes | `sent` |
| the connection lost, a timeout or an unreadable answer after the message started | unknown | **never resent**: `unknown_outcome` |

The attempt is committed (`attempts + 1`, `attempted_at`, `unclear = true`,
the same compare-and-set as Gmail's, so of two runners of one send only one
sends) **after** the server answered `DATA` with 354 and before the first byte
of the message, timed at that moment (after the blobs were read and the
session opened), so the quiet window below starts when the message can
start to go. A crash before then leaves the send provably unsent; a crash
after leaves it unclear. A runner that loses the compare-and-set there is in
the middle of `DATA`: it drops the connection without a word (no `QUIT`,
which the server would take as part of the message, and no end of data), so
the server delivers nothing of it.

What an unclear send becomes:

| | An unclear IMAP send |
|---|---|
| Send again | may deliver twice; never |
| Look for the `Message-ID` in Sent, then decide | only a server that keeps a copy of what it sends can prove anything, and then only "sent": "not found" proves nothing on any server (a copy may be written later, or never). Our own APPEND is made only after a 250, so for most servers Sent never has it |
| **Settle it `unknown_outcome` at once** (chosen) | the member is told it may have gone and checks; if the server did keep a copy, the reader brings it into the conversation like any message in Sent |

An unclear send is settled without contacting the server, but only once the
attempt that made it unclear has surely ended: no IMAP attempt starts before
`…__SEND_TIMEOUT_SECONDS` plus 30 seconds after the previous one (Gmail's and
Zoho's keep the HTTP timeout plus 30 seconds). So a runner that lost its
lease mid-message has finished (and, on a 250, recorded the send `sent`)
before another settles it.

### 6. Limits

Through the shared token buckets (`connectors.provider_budgets`), all taken
before connecting, each refilled continuously:

| Budget | Key | Default | Setting |
|---|---|---|---|
| messages per mailbox login | `smtp:login:<keyed digest of username and SMTP host>` | 50 an hour | `…__SEND_PER_HOUR` |
| recipients per mailbox login | `smtp:recipients:<same digest>` | 500 a day | `…__SEND_RECIPIENTS_PER_DAY` |
| messages per workspace | `smtp:workspace:<workspace id>` | 200 an hour | `…__SEND_WORKSPACE_PER_HOUR` |
| messages per SMTP host, every workspace (not a big provider's) | `smtp:host:<keyed digest of the host>` | 1,000 an hour | `…__SEND_HOST_PER_HOUR` |
| one workspace's share of that host (not a big provider's) | `smtp:host:<digest>:<workspace id>` | half of the host's | (derived) |
| recipients per message | (a cap, no bucket) | 50 | `…__SEND_RECIPIENTS_PER_MESSAGE` |

| | The SMTP host's own budget |
|---|---|
| One instance-wide cap for every host | one workspace can spend a host's hour for everyone, and a big provider's host (millions of accounts) is capped as a small one |
| **No host cap for a short list of big providers' submission hosts (`imap::SMTP_PROVIDER_HOSTS`, matched exactly), which limit each account themselves; on every other host the cap, of which one workspace takes at most half** (chosen) | protects a small or self-hosted server and every workspace on it, as the IDLE and run caps do (#318) |

The login's budgets are keyed by the login (the instance signer's HMAC,
like the IDLE leases' keys), not by the connection: removing and adding the
mailbox again, or connecting it in several workspaces, gives it no new
allowance, and no address or host name is stored in the shared table. The
defaults sit below the daily and hourly limits Yahoo, iCloud, Fastmail and
GMX publish for their free plans, so a member's busy day is not slowed but a
runaway never trips a provider's lock on the member's account. A send that
finds a budget empty waits for it (`retry_after`); one that waits past the
give-up check ends `unavailable` (nothing was sent).

Size, checked before anything is sent:

- the message on the wire (Bcc left out) must be at most
  `…__SEND_MAX_MESSAGE_BYTES` (25 MiB; at most the 35 MiB every send path
  allows), else `too_large` before connecting;
- when the server's EHLO announces `SIZE n` (n > 0), a larger message is
  refused before `MAIL` (`too_large`); `MAIL FROM` carries `SIZE=<bytes>`
  where offered.

### 7. Sessions

| | SMTP sessions and the IMAP session caps |
|---|---|
| Count them against #311/#318's IMAP caps | they reach a different server and service, which providers limit on their own (and often on another host); an SMTP session would take a run's or an IDLE's slot for nothing |
| **Separate** (chosen) | one SMTP session per mailbox login at a time (a slot of `smtp:session:<login digest>`, given back when the session ends, refilled within 15 minutes if a process dies holding it), and the hourly budgets above per login, workspace and host |

A send that finds its login's session slot taken retries in 10 seconds.

### 8. The Sent copy: made by the stream's run, after its read

| | Where the APPEND to Sent is made |
|---|---|
| In the send job, right after the 250 | a third IMAP session per login (beside the run's and IDLE's, #318's "one login holds one IDLE and its stream's one run"), and a second writer of the stream's locations racing the run |
| The stream's next run, before the read | one session and one writer, but a copy that will not go (a server that hangs up on `APPEND`) would stop inbound mail with it |
| **The stream's next run, over its session, once the read is done** (chosen) | one session, one writer of the locations (the run's ordering key); copy trouble is logged and never the run's, so inbound mail is always read; the next read finds the copy where it was recorded; the run is due at once (the send made it due) |

So a successful IMAP send records, in the job's transaction, the message's
Sent copy as `pending` with the bytes it will append (the message as built
from the whole document, Bcc kept), held as a blob owned by the send's copy
(`connectors.sent_copy`) until the copy ends. The stream's run, signed in:

1. takes at most 10 pending copies of its stream (oldest first), and stops
   starting new ones once it appended `…__RUN_BUDGET_BYTES` (after one at
   least);
2. finds the Sent folder: the SPECIAL-USE `\Sent` folder, else the folder
   the reader found by name (#311's name table). None in the folder list:
   `skipped` (`no_sent_folder`). The folders not listed yet (the very first
   run): the copy waits;
3. checks the bytes against the server's `APPENDLIMIT=n` (RFC 7889, when
   the capability names a number) and `…__MAX_MESSAGE_BYTES` (what the reader
   would download): larger, `skipped` (`too_large`);
4. looks in Sent first (below): already there, `found`, nothing appended. A
   Sent folder that will not open (`EXAMINE` or the search refused) cannot
   say whether a copy is there (an earlier `APPEND` cut short may have gone
   through): nothing is appended, and the copy waits, counted as trouble;
5. `APPEND <Sent> (\Seen) {n}`: a synchronising literal, its size announced
   and the bytes sent only after the server's `+` (a server that refuses the
   size takes none of it), through the session's bounded framer (each answer
   at most 64 KiB, at most 100 untagged responses, `TIMEOUT_SECONDS` × 2 plus
   a second per 256 KiB);
6. leaves Sent with `UNSELECT` when offered.

| APPEND's answer | The copy |
|---|---|
| `OK [APPENDUID v u]` (UIDPLUS) with v non-zero and, when the folder was read, its UIDVALIDITY | `appended`, located at Sent / v / u (`\Seen`), unless that place is recorded for another message (then left to the read) |
| `OK` without `APPENDUID` (or one that does not fit) | `appended`; the read finds the new UID by its `Message-ID` |
| `NO`/`BAD` with `[UNAVAILABLE]`, `[INUSE]` or `[LIMIT]` | stays `pending`; the run stops as busy (throttled), as a busy write-back does |
| any other `NO`/`BAD` (`[OVERQUOTA]`, `[TOOBIG]`, `[TRYCREATE]`…) | `failed` (`append_refused`), the response code logged when it is a known one |
| the session cut short or timed out | stays `pending`, the run fails as for a cut read |

A copy that stays `pending` through three runs (busy, cut, Sent not open)
is `failed` (`append_gave_up`). An APPEND's outcome never touches the send:
it stays `sent`, nothing is sent again, and nothing is retried but the APPEND
itself (always after a look, so a retried copy is never a second one). Each
copy's end is recorded **as it ends**, in a transaction of its own (the send's
copy state and reason, its blob released, its location), so a run cut short
by its time limit loses none of the copies it made. Trouble with copies (a
busy server, a cut session) ends the copies, not the run.

### 9. A server that keeps a copy itself

| | Whether to append |
|---|---|
| Always append | a duplicate in Sent at servers that keep their own copy (Gmail, Outlook, Yahoo do) |
| Never append; rely on the server | most IMAP servers (Dovecot, Cyrus, iCloud, Fastmail…) keep nothing: Sent stays empty |
| Search Sent after every send, append when not found | a server that writes its copy a moment later still gets a duplicate; one search per send |
| **A short list of SMTP hosts known to keep a copy, matched exactly, and, for every other host, a search of Sent for the `Message-ID` right before appending** (chosen) | the listed providers get no APPEND (`provider`); an unlisted server that keeps a copy at once gets no duplicate either (`found`); only one that keeps it late, and is not listed, can show two |

The list (`imap::SENT_KEEPERS`) names SMTP hosts of providers whose
submission servers file what they send: `smtp.gmail.com`,
`smtp.googlemail.com`, `smtp.mail.yahoo.com`, `smtp.aol.com`,
`smtp-mail.outlook.com`, `smtp.office365.com`. It changes only with a
release; a member cannot make a host "keep its copy" by typing it. The search
is `UID SEARCH HEADER Message-ID <id>` in Sent (opened read-only with
`EXAMINE`), the id as a quoted string; an id no quoted string can carry is
not searched (the copy is appended).

### 10. No echo, and the message shown at once

On the 250, the job indexes the sent message under the key the IMAP reader
gives it (`mid:` and the SHA-256 of its normalised `Message-ID`, #311), with
the tokens the reader would derive for a copy in Sent that the mailbox sent
(`SELF`, `SENT`, read), and announces it in a batch, in the same transaction
that marks the send `sent` (as Gmail's and Zoho's are). Then:

- the APPEND records the copy's location with `APPENDUID`; the next read
  finds the new UID in Sent, the same key at the same place, the same tokens:
  nothing emitted;
- without `APPENDUID`, the read finds the new UID by its `Message-ID`: a
  known key, a new location, the same tokens: nothing emitted;
- a server that keeps its own copy: the same, from the copy it made;
- IDLE watches the Inbox only (#318): an APPEND to Sent is never heard.

A message indexed this way has no location until its copy is found. The
reader's search for messages left with no location (#311) would remove it
once every folder had been read; a sent message whose copy is still
`pending`, or `skipped` or `failed`, is left out of that search, so the
member's own sent mail never disappears from its conversation because the
server would not take the copy.

### 11. Hostile and broken servers

- Every byte the SMTP server sends is bounded (1 MiB a session, `lettre`'s
  1,000-byte lines and 100,000-byte replies) and every step timed (section 1).
- The IMAP side of the copy goes through the session's bounded framer (#311),
  `APPENDUID` read only as two numbers.
- Logs carry ids, reply and response codes and counts: never a server's
  text, an address, a subject or a body. Job errors are fixed wording.
- Header injection: the wire message and the Sent copy come from the
  outgoing-document MIME builder; a line break in the subject or in a display
  name becomes a space, so no header can be added.

### 12. Reading less before refusing

The message's size is checked against `…__SEND_MAX_MESSAGE_BYTES` from the
sizes its blobs declare before any attachment is read into memory, and again
as each is read.

## Data ownership and events

Schema `connectors`. Migration `20261102010000_connectors__imap_send.sql`
extends `connectors.sends` (row-level security already enabled and forced,
the `workspace_isolation` policy, `SELECT, INSERT, UPDATE, DELETE` to
`campbooks_app`; unchanged):

- `sent_copy text NULL CHECK (IN 'pending', 'appended', 'found', 'provider',
  'skipped', 'failed')`: what became of the sent message's copy in Sent (NULL
  for Gmail and Zoho sends, and every send before this change);
- `sent_copy_raw jsonb NULL`: the blob reference of the bytes to append,
  while `pending` only (a CHECK);
- `sent_copy_reason text NULL`: a stable code (`no_sent_folder`,
  `too_large`, `append_refused`, `append_gave_up`, `copy_unreadable`);
- `sent_copy_attempts integer NOT NULL DEFAULT 0`;
- a partial index on `(stream_id, provider_message_id)` where the copy is
  `pending`, `skipped` or `failed` (the run's queue, and the reader's
  exception).

For an IMAP send, `provider_message_id` is the index key (`mid:…`), never an
address. Rows go with their send (and the send's stream is set to NULL when
the stream goes, as before). No event is added or changed:
`connectors.send_completed` keeps its closed set of reasons (`rejected`,
`too_large`, `needs_reauth`, `unavailable`, `unknown_outcome`); the sent copy
is not announced (the message itself is, as a change). No endpoint, scope,
job, schedule or handler is added.

## Tracked events

None added or changed: no new operation, job, schedule or handler, so no new
Coverage row. `connectors.send_message` and `connectors.sync_stream` already
have theirs.

## Settings

`CAMPBOOKS_CONNECTORS__IMAP__*` gains, each checked at startup **only while
`…__ENABLED` is on** (a stray value never stops an instance that does not read
IMAP), and none tied to the poll bounds:

| Setting | Default | Bounds |
|---|---|---|
| `SEND_PER_HOUR` | 50 | 1 to 10,000 |
| `SEND_RECIPIENTS_PER_MESSAGE` | 50 | 1 to 1,000 |
| `SEND_RECIPIENTS_PER_DAY` | 500 | `SEND_RECIPIENTS_PER_MESSAGE` to 100,000 |
| `SEND_WORKSPACE_PER_HOUR` | 200 | 1 to 100,000 |
| `SEND_HOST_PER_HOUR` | 1,000 | 1 to 1,000,000 |
| `SEND_MAX_MESSAGE_BYTES` | 25 MiB | 64 KiB to 35 MiB |
| `SEND_TIMEOUT_SECONDS` | 300 | `TIMEOUT_SECONDS` to 600 (within the 15-minute session slot) |

## Risks

- **A wrong curated list.** A listed provider that stops keeping copies
  leaves Sent empty; an unlisted one that keeps them late shows two. Both
  are visible to the member, neither loses or repeats mail.
- **Mailboxes whose username is not their address** cannot send until the
  connect screen's address check (#314) lands.
- **An SMTP password that differs from the IMAP one** is not caught by the
  shared refusal count while reads succeed (section 2).
- **Unknown outcomes are final.** A connection cut after the message went
  out is reported as "may have been sent" even when it was not.
