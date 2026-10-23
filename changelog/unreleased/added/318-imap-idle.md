- **New mail in IMAP mailboxes arrives as it lands.** Where the mail server offers IDLE, a
  worker keeps the mailbox's Inbox open and reads it again the moment the server says mail
  arrived, was deleted or changed, at most once every 30 seconds per mailbox however chatty
  the server is; the mailbox is still polled now and then for its other folders. Mailboxes
  whose server has no IDLE, and mailboxes quiet for two weeks, are polled as before. Each
  mailbox is held by one worker at a time, a changed password still asks the member to connect
  again after three refusals, and a server that misbehaves is dropped and retried later. Big
  providers' mail servers (from the bundled provider database) are no longer limited to four
  Campbooks sessions shared by every customer (`CAMPBOOKS_CONNECTORS__IMAP__PROVIDER_SESSIONS_PER_HOST`,
  200). Off by default (`CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED`, on top of
  `CAMPBOOKS_CONNECTORS__IMAP__ENABLED`).
