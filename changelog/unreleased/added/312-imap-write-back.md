- **What you do in Campbooks reaches your IMAP mailbox.** Marking mail read or unread,
  starring it, archiving, unarchiving, putting it in the trash or spam and taking it out
  again are made at the mail server, so any other mail app shows the same mailbox. Streams
  are mirrored as folders: Primary is the Inbox, and Updates, Receipts and Newsletters get a
  `Campbooks/<Stream>` folder, created (and subscribed) the first time one is needed, at most
  `CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS` (10) per mailbox; mail you file into one
  of those folders in another app moves its conversation to that stream. Messages are moved
  with MOVE where the server offers it, else copied and removed by their own UIDs only, so a
  message you marked deleted in another app is never expunged with them; a server that can do
  neither keeps the message where it is. Nothing is ever deleted for good, no folder is ever
  removed or renamed, and a change the server made since Campbooks last looked wins. Label
  changes are not made at IMAP servers yet. Still off by default
  (`CAMPBOOKS_CONNECTORS__IMAP__ENABLED`).
