# instance-mail Specification

## Purpose
The mail the instance sends on its own (security notices first, invitations
later): one SMTP transport the operator configures, or none, sent from jobs,
never leaking its credentials or the recipients' addresses into logs.

## Requirements

### Requirement: Instance mail transport
The server SHALL send its own mail through the SMTP server named by
`CAMPBOOKS_MAIL__SMTP_URL` (or the file `CAMPBOOKS_MAIL__SMTP_URL_FILE`),
`smtps://` or `smtp://` with TLS, from `CAMPBOOKS_MAIL__FROM`. Both unset SHALL
mean no mail is sent; one without the other, an unreadable file, a URL that
is not SMTP or a sender that is not an address SHALL stop startup with an
error naming the setting and never repeating the URL. The URL SHALL never be
logged. Mail SHALL be sent from jobs, never inline in a request; a failure
SHALL be recorded without the address or the server's reply. Every message
SHALL have a plain-text and an HTML part saying the same, with no images or
remote content.

#### Scenario: Not configured
- **WHEN** an instance starts without `CAMPBOOKS_MAIL__*`
- **THEN** it starts, sends no mail, and logs that notices are shown in the app only

#### Scenario: Half configured
- **WHEN** `CAMPBOOKS_MAIL__SMTP_URL=smtp://user:hunter2@smtp.example.com` is set without `CAMPBOOKS_MAIL__FROM`
- **THEN** startup fails with an error naming `CAMPBOOKS_MAIL__FROM` that does not contain `hunter2`

#### Scenario: A message
- **WHEN** the instance sends a notice through an SMTP server
- **THEN** the server receives one message from the configured sender to the recipient, with `text/plain` and `text/html` parts
