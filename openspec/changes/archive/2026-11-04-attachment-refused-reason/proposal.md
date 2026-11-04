## Why

When Zoho refused an attachment upload, the send ended `rejected` and the
composer said "Your mail provider refused this message. Check the addresses
and try again." The addresses had nothing to do with it, so the member went
looking in the wrong place. This is what happened with the 415 fixed in
`fix-zoho-attachment-upload` (campbooks-cloud#423).

Tracking: https://github.com/notacamp/campbooks-cloud/issues/424

## What Changes

- A new send failure reason, `attachment_refused`. Zoho's send fails with it
  when an attachment upload gets a definite refusal (a rate limit or a refused
  token still retries, as before). It travels the existing path: the
  connector's `connectors.send_completed` v1 (an added enum value, since the
  reason is a string to its consumer), then the draft's `failure_reason` and
  `conversations.send_failed`.
- The composer says it in words, in en, pt-PT, es-ES and fr: the mail
  provider wouldn't accept one of the attachments, so nothing was sent; try
  again, or try without the attachments.
- Tracking counts it as `rejected` (no tracking-plan change).
- Gmail, IMAP and Microsoft build the attachments into the message itself,
  so only Zoho's upload step uses the new reason.
