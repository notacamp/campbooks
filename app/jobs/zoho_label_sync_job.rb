class ZohoLabelSyncJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3
  discard_on AuthenticationError # Permanent auth failures

  def perform(email_account_id = nil)
    accounts = if email_account_id
      EmailAccount.where(id: email_account_id).active
    else
      EmailAccount.active
    end

    accounts.each do |account|
      service = if account.google?
        Google::LabelSyncService
      elsif account.zoho?
        Zoho::LabelSyncService
      else
        next # Microsoft/other providers have no label sync service yet
      end
      count = service.new(account).sync_labels!
      Rails.logger.info("[LabelSyncJob] Synced #{count} labels for #{account.email_address}")
    rescue => e
      Rails.logger.error("[LabelSyncJob] Sync failed for account #{account.id}: #{e.message}")
    end
  end
end
