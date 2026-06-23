class ZohoDriveAccountsController < ApplicationController
  before_action :require_authentication
  before_action :require_admin, only: [ :destroy ]

  def new
  end

  def create
    # Signed + user-bound so the callback can't be CSRF'd into linking an
    # attacker's Drive to a victim's workspace (was an unsigned JSON blob).
    state = Oauth::State.encode(flow: "drive_link", user_id: Current.user.id)

    auth_url = "https://accounts.zoho.eu/oauth/v2/auth"
    params = {
      client_id: ENV.fetch("ZOHO_CLIENT_ID"),
      response_type: "code",
      redirect_uri: oauth_zoho_callback_url,
      scope: "ZohoWorkDrive.files.CREATE,ZohoWorkDrive.files.READ",
      access_type: "offline",
      prompt: "consent",
      state: state
    }

    redirect_to "#{auth_url}?#{params.to_query}", allow_other_host: true
  end

  def destroy
    account = Current.workspace.zoho_drive_accounts.find(params[:id])
    account.deactivate!
    redirect_to settings_integrations_drive_path, success: t(".disconnected", name: account.email_address)
  end

  private

  def require_admin
    unless Current.user&.admin?
      redirect_to root_path, error: t("admin.base.no_permission")
    end
  end
end
