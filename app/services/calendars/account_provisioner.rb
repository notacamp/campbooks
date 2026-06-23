module Calendars
  # Provisions (or refreshes) the calendar side of a combined mail+calendar OAuth
  # grant. Called from the email OAuth callbacks after the EmailAccount is saved,
  # so one connect yields both the mailbox and its calendar from the same token.
  #
  # The CalendarAccount stores the same refresh token as the EmailAccount (one
  # grant); they share the access-token cache, so a single refresh serves both.
  class AccountProvisioner
    SUPPORTED = %i[google zoho].freeze # Microsoft has no calendar provider in v1

    def self.call(email_address:, provider:, refresh_token:, workspace:, owner:, provider_account_id: nil)
      return unless SUPPORTED.include?(provider.to_sym)
      return if refresh_token.blank?

      account = CalendarAccount.find_or_initialize_by(email_address: email_address, provider: provider)
      account.update!(
        refresh_token: refresh_token,
        provider_account_id: provider_account_id,
        workspace: workspace,
        active: true
      )
      account.calendar_account_users.find_or_create_by!(user: owner) do |entry|
        entry.owner = true
        entry.can_read = true
        entry.can_write = true
        entry.can_manage = true
      end

      # Discover this account's calendars and pull events on a fresh full sync.
      CalendarScanJob.perform_later(account.id, "full")
      account
    rescue => e
      # A calendar hiccup must never break the mailbox connect it rides on.
      Rails.logger.error("[Calendars::AccountProvisioner] failed for #{email_address}: #{e.class}: #{e.message}")
      nil
    end
  end
end
