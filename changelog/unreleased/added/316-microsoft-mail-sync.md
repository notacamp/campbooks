- **Outlook and Microsoft 365 mail is read (off by default).** Where the instance turns Microsoft
  mail on (`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED=true`), a connected Outlook.com, Hotmail, Live,
  MSN or Microsoft 365 mailbox now shows its mail: the Inbox, Sent Items, Junk Email, Deleted Items,
  Archive and your own folders (never Drafts), first the last 90 days and then what changes, a
  message moved in Outlook staying the same message. Your own folders and Outlook categories show
  as labels, and a `Campbooks/Updates` folder files its mail in that stream. Reading never changes
  anything in the mailbox: writing back and sending come next. With
  `CAMPBOOKS_CONNECTORS__MICROSOFT__NOTIFICATIONS=true` and a public `https` address, new mail is
  read as it arrives; otherwise the mailbox is checked as other mailboxes are.
