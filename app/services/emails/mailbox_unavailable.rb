module Emails
  # Raised when a Google account has no Gmail mailbox provisioned: the API answers
  # users.* calls with 400 FAILED_PRECONDITION "Mail service not enabled". A
  # permanent condition (a login-only / non-Gmail Google identity), so the scan
  # pipeline deactivates the account instead of retrying every minute. Mirrors
  # Emails::CursorExpired.
  class MailboxUnavailable < StandardError; end
end
