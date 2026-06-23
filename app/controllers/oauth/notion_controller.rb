class Oauth::NotionController < ApplicationController
  before_action :require_authentication

  def connect
    unless Notion::OauthClient.configured?
      redirect_to settings_integrations_notion_path, error: t(".not_configured") and return
    end

    state = Oauth::State.encode(
      flow: "notion_connect",
      user_id: Current.user.id,
      workspace_id: Current.workspace.id
    )
    redirect_to Notion::OauthClient.new.authorization_url(oauth_notion_callback_url, state),
                allow_other_host: true
  end

  def callback
    state = Oauth::State.decode(params[:state])
    unless state["verified"] && state["user_id"] == Current.user&.id
      redirect_to settings_integrations_notion_path, error: t(".invalid_state") and return
    end

    code = params.require(:code)
    data = Notion::OauthClient.new.exchange_code(code, oauth_notion_callback_url)

    integration = Current.workspace.notion_integrations
      .find_or_initialize_by(notion_workspace_id: data["workspace_id"])
    integration.update!(
      workspace: Current.workspace,
      access_token: data["access_token"],
      notion_workspace_name: data["workspace_name"],
      notion_workspace_icon: data["workspace_icon"],
      bot_id: data["bot_id"],
      authorized_by_user_id: Current.user.id,
      active: true
    )

    redirect_to settings_integrations_notion_path,
                success: t(".success", workspace: data["workspace_name"].presence || "Notion")
  rescue Notion::OauthClient::Error => e
    redirect_to settings_integrations_notion_path, error: t(".error", message: e.message)
  end
end
