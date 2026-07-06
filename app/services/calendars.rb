# Namespace for calendar sync services (EventWriter, jobs) and the shared errors
# the inbound/outbound paths raise.
module Calendars
  # A provider sync token is no longer valid (Google HTTP 410). The sync job
  # recovers by discarding the token and running a full re-sync.
  class SyncTokenExpired < StandardError; end

  # A write lost its etag precondition (HTTP 412): the event changed remotely
  # since we loaded it. EventWriter re-fetches, re-applies, and retries.
  class ConflictError < StandardError; end

  # The connected account has no Google Calendar provisioned (403 "The user must
  # be signed up for Google Calendar") — a login-only / mail-only identity. A
  # permanent condition, so CalendarScanJob deactivates the account instead of
  # re-listing every minute.
  class ServiceUnavailable < StandardError; end
end
