- **Connect Outlook and Microsoft 365 mailboxes (off by default).** Where the instance turns
  Microsoft mail on (`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED=true`), Add a mailbox sends an
  Outlook.com, Hotmail, Live, MSN or Microsoft 365 address to Microsoft's own sign-in, "Connect
  Outlook", with "Also use this account to sign in", just like Gmail. Personal accounts connect as
  well as work and school ones (tenant `common`, `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT`), with the
  app of Sign in with Microsoft; reconnecting with another account is refused, and a grant
  Microsoft no longer honours asks you to sign in again. Reading the mail comes next: until then a
  connected Outlook mailbox shows none. While the flag is off, nothing changes: Outlook is still
  coming soon, whatever else is set.
