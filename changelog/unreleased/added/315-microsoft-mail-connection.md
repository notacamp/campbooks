- **Connect Outlook and Microsoft 365 mailboxes (off by default).** Where the instance turns
  Microsoft mail on (`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED=true`), Add a mailbox sends an
  Outlook.com, Hotmail, Live, MSN or Microsoft 365 address to Microsoft's own sign-in, "Connect
  Outlook", with "Also use this account to sign in", just like Gmail, with the app of Sign in with
  Microsoft and at its tenant (`CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT=common` to also connect
  personal accounts where sign-in is for work accounts only). Signing in with Microsoft never asks
  for mail, except once for a new account. Reconnecting with another account is refused, and a grant
  Microsoft no longer honours asks you to sign in again. Reading the mail comes next: until then a
  connected Outlook mailbox shows none. While the flag is off, nothing changes: Outlook is still
  coming soon, whatever else is set.
