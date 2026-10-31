- **New Zoho mail shows up within about a minute.** Zoho mailboxes are now checked every
  minute (it could take up to three), and Gmail and calendar changes no longer wait an
  extra minute between checks. Self-hosters can change how long Zoho mail may wait with
  `CAMPBOOKS_CONNECTORS__ZOHO__POLL_MAX_SECONDS` (30 to 14400 seconds, default 60); a value
  that is not usable is ignored with a warning in the log.
