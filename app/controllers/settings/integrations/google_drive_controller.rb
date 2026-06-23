class Settings::Integrations::GoogleDriveController < Settings::BaseController
  def show
    @account = Current.workspace.google_drive_accounts.connected.first
    @document_types = Current.workspace.document_types.order(:name)
    @configs = GoogleDriveConfig.includes(:document_type).index_by(&:document_type_id)
  end

  def destroy
    account = Current.workspace.google_drive_accounts.connected.first
    account&.deactivate!
    redirect_to settings_integrations_path, success: t(".disconnected")
  end

  private

  def current_section
    "integrations"
  end
end
