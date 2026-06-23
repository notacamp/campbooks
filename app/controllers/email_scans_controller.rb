class EmailScansController < ApplicationController
  before_action :require_authentication

  # Individual scan detail — deep-linked from the Accounts panel's recent scans.
  # The list and "scan now" action now live in the inbox settings Accounts panel.
  def show
    readable_ids = Current.user.readable_email_accounts.pluck(:id)
    @scan_log = EmailScanLog.where(email_account_id: readable_ids).includes(:email_account).find(params[:id])
    @email_messages = @scan_log.email_messages.order(received_at: :desc)
  end
end
