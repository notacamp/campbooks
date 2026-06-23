# The OAuth grant is genuinely, permanently dead — the provider says the refresh
# token itself is revoked/expired (`invalid_grant`; Zoho `invalid_code`). ONLY
# this warrants deactivating the account (the owner must reconnect via OAuth);
# ambiguous/transient failures stay as the AuthenticationError base class.
#
# Subclass of AuthenticationError so existing `rescue AuthenticationError` /
# `discard_on AuthenticationError` paths still catch it.
class PermanentAuthError < AuthenticationError; end
