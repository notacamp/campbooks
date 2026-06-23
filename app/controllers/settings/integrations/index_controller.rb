class Settings::Integrations::IndexController < Settings::BaseController
  def show
    @google_drive_connected = Current.workspace.google_drive_accounts.connected.exists?
    @notion_connected = Current.workspace.notion_integrations.active.exists?
    @zoho_drive_connected = ZohoDriveAccount.active.exists?
    @calendar_connected = Current.user.calendar_accounts.active.exists?
    @connections_count = Current.workspace.connections.count
  end

  private

  def current_section
    "integrations"
  end
end
