- **IMAP mailboxes are read.** A connected IMAP mailbox fills the inbox, Sent, Spam and
  Trash like a Gmail or Zoho one: the last 90 days first, newest first, then only the
  folders that changed, with the flags of the window read again every six hours. A message
  is one message wherever it is filed, so moving or copying it in another mail app never
  duplicates it, and the member's own folders show as labels. Reading never marks mail
  read. A password refused three runs in a row asks the member to connect again; a busy
  server only slows the mailbox down; each mail server sees at most four Campbooks sessions
  at once, two of them per workspace (`CAMPBOOKS_CONNECTORS__IMAP__SESSIONS_PER_HOST`,
  `…__WORKSPACE_SESSIONS_PER_HOST`); a folder the server will not open is set aside for an
  hour. Still off by default
  (`CAMPBOOKS_CONNECTORS__IMAP__ENABLED`) until the connect screen lands.
