module Emails
  # A provider delta cursor is no longer valid: a Gmail historyId too old (HTTP
  # 404) or a Microsoft Graph deltaToken expired (HTTP 410). The engine recovers
  # by discarding the cursor and scheduling a full re-baseline
  # (Emails::FullResyncJob), mirroring Calendars::SyncTokenExpired.
  #
  # Lives in its own file (rather than an Emails-namespace file) so it autoloads
  # reliably: Emails is an implicit namespace owned by the app/services/emails/
  # directory, and a constant referenced in `rescue` clauses must never depend on
  # load order.
  class CursorExpired < StandardError; end
end
