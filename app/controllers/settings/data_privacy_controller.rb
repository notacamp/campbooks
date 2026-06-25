class Settings::DataPrivacyController < Settings::BaseController
  before_action :set_org

  # A privacy-framed overview that hosts the global AI kill-switch and surfaces
  # (read-only) which provider + region handles each AI task and which third
  # parties receive workspace data. The detailed controls stay where they live —
  # this page links out to Settings → AI, Integrations, and Account.
  def show
    @configs = @org.ai_configurations.includes(:ai_adapter).order(:purpose).index_by(&:purpose)

    @email_accounts         = @org.email_accounts.order(:email_address)
    @google_drive_connected = @org.google_drive_accounts.connected.exists?
    @notion_connected       = @org.notion_integrations.active.exists?
    @zoho_drive_connected   = @org.zoho_drive_accounts.active.exists?
    @calendar_connected     = current_user.calendar_accounts.active.exists?
  end

  def update
    if @org.update(data_privacy_params)
      redirect_to settings_data_privacy_path, success: t(".updated")
    else
      redirect_to settings_data_privacy_path, alert: t(".update_failed")
    end
  end

  private

  def set_org
    @org = Current.workspace || current_user&.workspace
  end

  def data_privacy_params
    params.permit(:ai_processing_enabled)
  end

  def current_section
    "data_privacy"
  end
end
