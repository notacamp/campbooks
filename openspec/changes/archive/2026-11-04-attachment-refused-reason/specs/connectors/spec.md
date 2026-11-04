## MODIFIED Requirements

### Requirement: Sending through Zoho
A send from an inbox fed by a Zoho stream SHALL go through Zoho's send API
from the stream's mailbox, with To, Cc, Bcc, subject, the HTML alternative
(or the text) and the attachments, uploaded first. Each attachment SHALL be
uploaded as a raw stream (`application/octet-stream`) named by its file name,
whatever its own media type. An attachment Zoho refuses with a definite
answer (not a rate limit or a refused token) SHALL fail the send
`attachment_refused`, and nothing SHALL be sent. A reply whose
`In-Reply-To` names a message of the stream SHALL be sent as Zoho's reply to
that message. The idempotency rules of Gmail sending SHALL hold, with one
difference: while a send is unclear, the next attempt SHALL look in the
mailbox's Sent folder and Outbox for a message sent between 30 seconds
before that attempt and the HTTP timeout plus 30 seconds after it, with
the same subject, To, Cc, attachment flag and start of text, not already
another send's. Exactly one such message SHALL record the send as sent;
none SHALL prove it was not sent; several, a copy without a time, or a
mailbox whose Sent folder cannot be found SHALL leave it unclear. When
Zoho's answer to a send names no message, the job SHALL claim the Sent
copy it finds this way, so no later look-up takes it for another send. Once Zoho accepts, the sent message SHALL be fetched, indexed and
announced in the job's transaction like Gmail's; when it cannot be fetched
the send SHALL still be `sent` and the stream due at once.

#### Scenario: A reply goes out as a Zoho reply
- **WHEN** a member replies to a message of a Zoho inbox
- **THEN** Zoho receives one reply call naming that message, and the sent copy joins the same conversation

#### Scenario: A timeout after Zoho accepted the message
- **WHEN** Zoho accepts a message but the answer is lost, and the job runs again
- **THEN** the job finds it in the Sent folder and Zoho receives no second send

#### Scenario: A follow-up with the same subject
- **WHEN** an attempt never reached Zoho and, two minutes later, the member sends another message with the same subject and recipient from another client
- **THEN** that message is not taken for the send, which is sent

#### Scenario: Two matching copies
- **WHEN** two sent messages match an unclear attempt
- **THEN** the send stays unclear, is never sent again, and ends `unknown_outcome`

#### Scenario: A reply with attachments
- **WHEN** a member replies from a Zoho inbox with a PDF and a JPEG attached
- **THEN** Zoho receives both files as octet streams under their file names, then one send naming both, and the send is `sent`

#### Scenario: Zoho refuses an attachment
- **WHEN** Zoho answers 400 to an attachment upload of a reply
- **THEN** Zoho receives no send, and the send fails `attachment_refused`, not `rejected`
