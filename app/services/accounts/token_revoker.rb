module Accounts
  # Revokes an OAuth account's grant at the provider on USER-INITIATED disconnect,
  # but only when no other ACTIVE account in the workspace shares the same refresh
  # token. Google/Zoho calendar accounts are provisioned sharing their sibling
  # mailbox's grant (see CLAUDE.md "Calendar"), so an unconditional revoke would
  # break the still-connected sibling. refresh_token is non-deterministically
  # encrypted, so sharing is detected by comparing decrypted values in Ruby.
  #
  # Call AFTER #deactivate! so `account` is no longer in the active set. Best-effort:
  # never raises. (Full account deletion uses Accounts::Deleter#revoke_tokens_best_effort,
  # which revokes everything — there the whole workspace is torn down, so sharing is moot.)
  class TokenRevoker
    def self.revoke_if_unshared(account)
      token = account.refresh_token
      return false if token.blank?

      workspace = account.workspace
      return false unless workspace

      sharers = workspace.email_accounts.active.to_a + workspace.calendar_accounts.active.to_a
      return false if sharers.any? { |other| other.refresh_token == token }

      client = account.oauth_client
      return false unless client.respond_to?(:revoke_token)

      client.revoke_token
    rescue StandardError => e
      Rails.logger.warn("[Accounts::TokenRevoker] best-effort revoke failed: #{e.class}: #{e.message}")
      false
    end
  end
end
