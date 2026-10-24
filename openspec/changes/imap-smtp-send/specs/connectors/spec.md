## ADDED Requirements

### Requirement: Sending through an IMAP mailbox's SMTP server
A send from an inbox fed by an IMAP stream SHALL go through the connection's
SMTP submission server, at the address the kernel's guard resolved and checked
for that send, on a mail port only: TLS from the first byte when the server is
set to `tls` (465), or STARTTLS when it is set to `starttls` (587), the
certificate checked against the host name by the platform verifier. On a
`starttls` server that does not offer STARTTLS the module SHALL say nothing
more (no `AUTH`, no password, no `MAIL`) and the send SHALL fail
`unavailable`. It SHALL sign in with `AUTH PLAIN` or `AUTH LOGIN` and the
connection's sealed password, opened for that send only. A refused password
(a 5xx answer to `AUTH`) SHALL count towards the stream's shared login
refusals, the third in a row setting `needs_reauth` and failing the send
`needs_reauth`; a busy answer (4xx) SHALL be retried and never counted. The
envelope sender and the `From` header SHALL be the connection's own address:
a document whose `From` address is any other SHALL not be sent (`rejected`).
The envelope recipients SHALL be the document's To, Cc and Bcc; the message
sent SHALL carry no `Bcc` header. A recipient the server refuses SHALL abort
the whole send before `DATA` (`rejected`). The message SHALL be built by the
outgoing-document MIME builder, so no line break in a subject or a display
name adds a header. Logs SHALL carry ids, reply codes and counts only.

#### Scenario: Sending over implicit TLS
- **WHEN** a member replies from an IMAP inbox whose SMTP server is set to `tls` on 465
- **THEN** the server receives the message over TLS with the document's `Message-ID`, `In-Reply-To` and `References`, and the send is `sent`

#### Scenario: Sending over STARTTLS
- **WHEN** the SMTP server is set to `starttls` on 587 and offers STARTTLS
- **THEN** the session is upgraded before `AUTH`, the message is delivered once and the send is `sent`

#### Scenario: No STARTTLS on 587
- **WHEN** the SMTP server set to `starttls` does not offer STARTTLS
- **THEN** no `AUTH` and no `MAIL` reach it, no password goes in the clear, and the send fails `unavailable`

#### Scenario: From another address
- **WHEN** a send's document is from an address that is not the connection's
- **THEN** no SMTP session is opened and the send fails `rejected`

#### Scenario: Bcc in the envelope only
- **WHEN** a message has a Bcc recipient
- **THEN** the server receives `RCPT TO` for that recipient, the message it receives has no `Bcc` header, and the Sent copy keeps it

#### Scenario: A refused password
- **WHEN** the SMTP server answers `AUTH` with 535
- **THEN** the refusal is counted with the stream's login refusals, the send is retried later, and after the third refusal in a row the connection needs the member and the send fails `needs_reauth`

#### Scenario: No header injection
- **WHEN** a document's subject and a recipient's display name hold a line break followed by `Bcc: someone`
- **THEN** the message sent has no `Bcc` header and the server receives no `RCPT TO` for that address

### Requirement: An IMAP send goes out at most once
The module SHALL record an IMAP send's attempt, on its own, after the server
accepted `DATA` (354) and before the first byte of the message. A failure
before then (connecting, TLS, the greeting, EHLO, `AUTH`, `MAIL`, `RCPT`,
`DATA`), and a 4xx or 5xx answer to the message itself, SHALL be definite:
retried later on a 4xx or no answer, `rejected` on a 5xx (`too_large` on
552). A lost connection, a timeout or an unreadable answer once the message
started SHALL never be followed by another attempt: the send SHALL end
`unknown_outcome`, once the attempt has surely ended (no IMAP attempt starts
sooner than `CAMPBOOKS_CONNECTORS__IMAP__SEND_TIMEOUT_SECONDS` plus 30
seconds after the previous one). Every step of a submission SHALL have a time
limit, and at most 1 MiB SHALL be read from the server in one session.

#### Scenario: Busy, then accepted
- **WHEN** the SMTP server answers the first `RCPT TO` with 451, then accepts the message on the next attempt
- **THEN** the message is delivered once and the send is `sent`

#### Scenario: Connection lost after the message
- **WHEN** the connection is lost after the message was sent and before the server's final answer
- **THEN** the server received one `DATA`, no second attempt is made, and the send fails `unknown_outcome`

#### Scenario: A server that never stops talking
- **WHEN** the SMTP server answers EHLO with more than 1 MiB and no end
- **THEN** the session ends within its bounds, nothing is sent, and the send is retried

### Requirement: IMAP send limits
Before connecting, an IMAP send SHALL take, from the shared budgets, one
message of its mailbox login's hourly allowance
(`CAMPBOOKS_CONNECTORS__IMAP__SEND_PER_HOUR`, default 50), its recipients
from the login's daily allowance (`…__SEND_RECIPIENTS_PER_DAY`, 500), one
message of its workspace's hourly allowance (`…__SEND_WORKSPACE_PER_HOUR`,
200) and one of its SMTP host's (`…__SEND_HOST_PER_HOUR`, 1,000), the
login's keyed by a digest of the login, never by the connection; an empty
budget SHALL make the send wait. A message with more recipients than
`…__SEND_RECIPIENTS_PER_MESSAGE` (50) SHALL fail `rejected`; one larger than
`…__SEND_MAX_MESSAGE_BYTES` (25 MiB), or than the `SIZE` the server announces
in EHLO, SHALL fail `too_large` before `MAIL`. One SMTP session per mailbox
login SHALL be open at a time.

#### Scenario: Larger than the server takes
- **WHEN** the server announces `SIZE` smaller than the message
- **THEN** the server receives no `MAIL FROM` and no `DATA`, and the send fails `too_large`

#### Scenario: Over the hourly allowance
- **WHEN** a mailbox login has used its messages for the hour
- **THEN** its next send waits for the budget and the server receives nothing until then

#### Scenario: Too many recipients
- **WHEN** a message names more recipients than `CAMPBOOKS_CONNECTORS__IMAP__SEND_RECIPIENTS_PER_MESSAGE`
- **THEN** no SMTP session is opened and the send fails `rejected`

### Requirement: The sent copy in an IMAP Sent folder
After an IMAP send is accepted, the job SHALL index the sent message under the
reader's key for its `Message-ID`, as a read copy in Sent that the mailbox
sent, and announce it, in the transaction that marks the send `sent`. Unless
its SMTP host is one of the providers known to keep a copy of what they send
(a short list in the code, matched exactly), the stream's next run SHALL,
after its write-backs and before it reads, look for the `Message-ID` in the
Sent folder and, when it is not there, `APPEND` the message (Bcc kept) with
`\Seen` to the SPECIAL-USE `\Sent` folder or the Sent folder the reader found
by name, as a synchronising literal within the server's `APPENDLIMIT` and
`CAMPBOOKS_CONNECTORS__IMAP__MAX_MESSAGE_BYTES`, through the session's
bounded framer. With `APPENDUID` the copy's location SHALL be recorded in the
run's transaction before the read, so neither the read nor IDLE announces the
message again. A busy answer or a cut session SHALL leave the copy for the
next run (three at most); any other refusal SHALL end it `failed`. Nothing
about the copy SHALL fail or repeat the send. A sent message whose copy could
not be made SHALL never be removed for having no location.

#### Scenario: Appended with APPENDUID and no echo
- **WHEN** a send is accepted by a server offering UIDPLUS, and the stream then runs twice
- **THEN** Sent holds one `\Seen` copy appended by Campbooks, its location is recorded, and neither run announces the message

#### Scenario: A provider that keeps its own copy
- **WHEN** the SMTP host is on the list of providers that keep a copy
- **THEN** no `APPEND` is sent and the copy the server keeps is read as the sent message, without echo

#### Scenario: Already in Sent
- **WHEN** the server put the message in Sent itself before the run
- **THEN** no `APPEND` is sent and the message is not announced again

#### Scenario: An APPEND the server refuses
- **WHEN** the server answers the `APPEND` with `NO [OVERQUOTA]`
- **THEN** the send stays `sent`, the server receives no second message, the copy is `failed`, and the sent message stays in its conversation

#### Scenario: Larger than the server's APPENDLIMIT
- **WHEN** the server's `APPENDLIMIT` is smaller than the message
- **THEN** no `APPEND` is sent and the copy is `skipped`

#### Scenario: Copies are isolated by workspace
- **WHEN** a transaction of one workspace reads the sends' copies
- **THEN** it sees none of another workspace's

### Requirement: IMAP send settings never stop a server that does not read IMAP
The server SHALL check the IMAP send settings (`…__SEND_PER_HOUR`,
`…__SEND_RECIPIENTS_PER_MESSAGE`, `…__SEND_RECIPIENTS_PER_DAY`,
`…__SEND_WORKSPACE_PER_HOUR`, `…__SEND_HOST_PER_HOUR`,
`…__SEND_MAX_MESSAGE_BYTES`, `…__SEND_TIMEOUT_SECONDS`) at startup only while
`CAMPBOOKS_CONNECTORS__IMAP__ENABLED` is on, and SHALL tie none of them to the
poll bounds. Their defaults SHALL start with any valid poll bounds.

#### Scenario: A stray value with IMAP off
- **WHEN** an instance with IMAP off sets `CAMPBOOKS_CONNECTORS__IMAP__SEND_PER_HOUR` to 0
- **THEN** the server starts

#### Scenario: Production's poll bounds
- **WHEN** an instance sets its poll bounds to 60–120 seconds, with IMAP off or on and the send defaults
- **THEN** the server starts
