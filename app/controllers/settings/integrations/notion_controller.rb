class Settings::Integrations::NotionController < Settings::BaseController
  def show
    load_show_data
    @new_integration = Current.workspace.notion_integrations.new
  end

  # Manual-token connection (fallback for self-hosted instances without OAuth creds).
  def update
    @new_integration = Current.workspace.notion_integrations.new(integration_params)

    unless @new_integration.save
      load_show_data
      return render :show, status: :unprocessable_entity
    end

    begin
      bot_info = Notion::Client.new(@new_integration).get_bot_info
      name = bot_info["workspace_name"] || bot_info.dig("bot", "workspace_name")
      id   = bot_info["workspace_id"] || bot_info.dig("bot", "workspace_id")
      @new_integration.update_columns(notion_workspace_name: name, notion_workspace_id: id) if name || id
      redirect_to settings_integrations_notion_path, success: t(".saved")
    rescue => e
      # Token didn't authenticate — don't keep a dead integration around.
      @new_integration.destroy
      load_show_data
      @new_integration = Current.workspace.notion_integrations.new
      flash.now[:error] = t(".workspace_warning", error: e.message)
      render :show, status: :unprocessable_entity
    end
  end

  def destroy
    integration = Current.workspace.notion_integrations.find(params[:id])
    integration.deactivate!
    redirect_to settings_integrations_notion_path, success: t(".disconnected")
  end

  private

  def current_section
    "integrations"
  end

  def load_show_data
    @oauth_configured = Notion::OauthClient.configured?
    @integrations = Current.workspace.notion_integrations.active.order(:created_at)
    @mappings = NotionDatabaseMapping.joins(:document_type)
      .where(document_types: { workspace_id: Current.workspace.id })
      .includes(:document_type).order(
        Arel.sql("(SELECT name FROM document_types WHERE document_types.id = notion_database_mappings.document_type_id)")
      )
  end

  def integration_params
    params.require(:notion_integration).permit(:access_token)
  end
end
