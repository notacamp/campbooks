- **Connect Yahoo, AOL, iCloud, Fastmail, GMX, Yandex and your own domain with an app
  password.** Settings → Mailboxes → Add a mailbox (⇧C) asks for the address first: Gmail and
  Zoho addresses go to their own sign-in, Outlook says it is coming soon, and every other
  mailbox gets its provider's app-password steps with a link to the provider's own help page
  (for GMX and web.de, turning on IMAP first), one password field, and Advanced servers
  (TLS or STARTTLS only) prefilled. Refusals are explained in plain words, never in the
  server's.
- **Verification is automatic.** A mailbox that signs in with a username other than its
  address gets one message with a one-time code, sent through its own SMTP server; Campbooks
  finds it in that mailbox's Inbox, marks the address verified (so it can send) and removes
  the message. You type nothing; Mailboxes says "Checking your address…", then whether it
  worked, with Try again.
- **IMAP is on by default** (`CAMPBOOKS_CONNECTORS__IMAP__ENABLED`; it needs the instance
  secret key, and without one IMAP is simply not offered). IDLE push stays off unless
  `CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED` is set.
