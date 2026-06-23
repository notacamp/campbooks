# Raised when an OAuth token refresh fails. Distinct from generic RuntimeError so
# jobs can `discard_on AuthenticationError` without also discarding unrelated
# runtime errors.
#
# This base class covers the AMBIGUOUS case — the refresh failed, but it may be
# transient or our own fault (provider 5xx, rate limiting, or a bad/missing
# client credential like `invalid_client` during a deploy). Callers must NOT
# treat it as "the user's grant is dead": doing so once disconnected every
# account at once when a cutover briefly served placeholder OAuth creds.
#
# The genuinely-dead-grant case is the PermanentAuthError subclass (own file).
class AuthenticationError < StandardError; end
