class AccountExportJob < ApplicationJob
  queue_as :default

  def perform(account_export_id)
    account_export = AccountExport.find(account_export_id)
    account_export.update!(status: :generating)

    begin
      zip_data = Accounts::ArchiveGenerator.new(account_export.user).call

      account_export.archive.attach(
        io: StringIO.new(zip_data),
        filename: "campbooks-data-export_#{account_export.created_at.strftime('%Y%m%d_%H%M%S')}.zip",
        content_type: "application/zip"
      )

      account_export.update!(status: :generated)
      Notifier.account_export_ready(account_export)
    rescue => e
      account_export.update!(status: :failed)
      Notifier.account_export_failed(account_export)
      Rails.logger.error("[AccountExportJob] Error: #{e.message}")
      raise
    end
  end
end
