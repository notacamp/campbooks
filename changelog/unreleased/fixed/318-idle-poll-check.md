- **The server starts again on instances with narrowed poll bounds.** The IMAP push setting
  for the safety poll (`CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS`) is now checked
  against the poll bounds only when IMAP and IDLE are both on. Before, an instance whose poll bounds
  left out 900 seconds refused to start even with IMAP off.
