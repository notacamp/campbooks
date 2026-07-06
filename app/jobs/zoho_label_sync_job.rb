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
    rescue Emails::MailboxUnavailable => e
      # No Gmail mailbox behind this Google identity — deactivate rather than
      # retry the doomed label fetch every cycle (see EmailScanJob).
      Rails.logger.info("[LabelSyncJob] #{account.email_address}: #{e.message} — disabling sync")
      account.deactivate_for!(:mail_service_unavailable)
    rescue => e
      # Isolate per-account failures so one bad mailbox doesn't abort the batch —
      # but surface them to error tracking, not only the log. A silent rescue here
      # is exactly how a mailbox's labels drift stale for months unnoticed.
      Rails.logger.error("[LabelSyncJob] Sync failed for account #{account.id}: #{e.message}")
      Rails.error.report(e, handled: true, context: { account_id: account.id, job: "ZohoLabelSyncJob" })
    end
  end
end
