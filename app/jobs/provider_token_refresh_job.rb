class ProviderTokenRefreshJob < ApplicationJob
  queue_as :default

  def perform
    refresh_accounts(EmailAccount.active)
    refresh_accounts(CalendarAccount.active)
  end

  private

  def refresh_accounts(scope)
    scope.find_each do |account|
      account.oauth_client.refresh!
      Rails.logger.info("[ProviderTokenRefreshJob] Token refreshed for #{account.email_address}")
    rescue PermanentAuthError => e
      # The grant is genuinely dead (provider says invalid_grant/invalid_code):
      # deactivate so we stop scanning it every minute. The owner must reconnect.
      account.deactivate!
      Rails.logger.warn("[ProviderTokenRefreshJob] Deactivated #{account.email_address} after permanent auth failure: #{e.message}")
    rescue AuthenticationError => e
      # Ambiguous/transient failure (provider 5xx, throttling, or a bad/missing
      # client credential during a deploy). Leave the account ACTIVE and just log:
      # a server-side blip must never disconnect every user's inbox at once. The
      # next run retries; a truly-dead grant will surface as PermanentAuthError.
      Rails.logger.warn("[ProviderTokenRefreshJob] Transient auth failure for #{account.email_address}, left active: #{e.message}")
    rescue => e
      Rails.logger.error("[ProviderTokenRefreshJob] Token refresh failed for #{account.email_address}: #{e.message}")
    end
  end
end
