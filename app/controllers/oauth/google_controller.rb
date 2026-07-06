class Oauth::GoogleController < ApplicationController
  before_action :require_authentication

  def connect
    unless GoogleDrive::OauthClient.configured?
      redirect_to settings_integrations_google_drive_path, warning: t(".not_configured")
      return
    end

    oauth = GoogleDrive::OauthClient.new
    redirect_to oauth.authorization_url(oauth_google_callback_url), allow_other_host: true
  end

  def callback
    code = params.require(:code)
    oauth = GoogleDrive::OauthClient.new
    token_data = oauth.exchange_code(code, oauth_google_callback_url)

    # Fetch user info
    email = fetch_email(token_data["access_token"])

    account = Current.workspace.google_drive_accounts.connected.first ||
              Current.workspace.google_drive_accounts.new
    account.update!(
      email: email,
      refresh_token: token_data["refresh_token"],
      connected: true,
      scopes: token_data["scope"],
      workspace: Current.workspace
    )

    redirect_to settings_integrations_google_drive_path, success: t(".success", email: email)
  rescue GoogleDrive::OauthError => e
    redirect_to settings_integrations_google_drive_path, error: t(".error", message: e.message)
  end

  private

  def fetch_email(access_token)
    response = SystemHealth.track(service: "google_drive_oauth", operation: "GET /oauth2/v2/userinfo") do
      Faraday.get("https://www.googleapis.com/oauth2/v2/userinfo") do |req|
        req.headers["Authorization"] = "Bearer #{access_token}"
      end
    end
    data = JSON.parse(response.body)
    data["email"]
  end
end
