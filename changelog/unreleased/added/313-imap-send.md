- **Send from your IMAP mailbox.** Replies and new mail from an IMAP inbox go out through the
  mailbox's own SMTP server (465 with TLS, or 587 with STARTTLS, which is required: no password
  or message ever goes in the clear), from the mailbox's own address only, with Bcc recipients in
  the envelope and never in the headers. A message is never sent twice: when the connection is
  lost after the message went out, the send says it may have been sent instead of trying again.
  The sent message shows in its conversation at once, and a copy is kept in your Sent folder
  (read, with Bcc) unless your provider files one itself or one is already there; a copy the
  server will not take never undoes the send. Sends are paced per mailbox (messages an hour,
  recipients a day), per workspace and per SMTP host, with conservative defaults set by
  `CAMPBOOKS_CONNECTORS__IMAP__SEND_*`, and a message larger than the server takes is refused
  before anything is sent. Still off by default (`CAMPBOOKS_CONNECTORS__IMAP__ENABLED`).
