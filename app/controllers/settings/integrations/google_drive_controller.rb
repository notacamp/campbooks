class Settings::Integrations::GoogleDriveController < Settings::BaseController
  def show
    @account = Current.workspace.google_drive_accounts.connected.first
    @oauth_configured = GoogleDrive::OauthClient.configured?
    @document_types = Current.workspace.document_types.order(:name)
    @configs = GoogleDriveConfig.includes(:document_type).index_by(&:document_type_id)
    @failed_count = Current.workspace.documents.where(google_drive_push_status: :failed).count if @account&.connected?
  end

  def destroy
    account = Current.workspace.google_drive_accounts.connected.first
    account&.deactivate!
    redirect_to settings_integrations_root_path, success: t(".disconnected")
  end

  def retry_failed
    failed = Current.workspace.documents.where(google_drive_push_status: :failed)
    count = failed.count

    if count > 0
      failed.find_each { |doc| GoogleDrivePushJob.perform_later(doc.id) }
      redirect_to settings_integrations_google_drive_path, success: t(".retrying", count: count)
    else
      redirect_to settings_integrations_google_drive_path, notice: t(".none_failed")
    end
  end

  private

  def current_section
    "integrations"
  end
end
