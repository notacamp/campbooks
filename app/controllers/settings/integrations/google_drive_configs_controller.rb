class Settings::Integrations::GoogleDriveConfigsController < Settings::BaseController
  before_action :set_config

  def edit
  end

  def update
    if @config.update(config_params)
      redirect_to settings_integrations_google_drive_path, success: t(".saved")
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def browse_folders
    @account = Current.workspace.google_drive_accounts.connected.first
    unless @account&.full_access?
      render plain: t(".reconnect_needed"), status: :forbidden and return
    end

    @parent_id = params[:parent_id].presence
    @current = (@parent_id && @parent_id != "root") ? client.get_folder(@parent_id) : nil
    @up_id = @current&.parents&.first || "root"
    @folders = client.list_folders(parent_id: @parent_id)
  rescue ::GoogleDrive::ApiError, Faraday::Error => e
    render plain: t(".browse_failed", message: e.message), status: :internal_server_error
  end

  private

  def current_section
    "integrations"
  end

  def set_config
    @config = GoogleDriveConfig.joins(:document_type)
      .where(document_types: { workspace_id: Current.workspace.id })
      .find_or_initialize_by(document_type_id: params[:document_type_id])
  end

  def config_params
    params.require(:google_drive_config).permit(
      :auto_push, :folder_id, :folder_path, :naming_pattern, :subfolder_pattern
    )
  end

  def client
    @client ||= ::GoogleDrive::Client.new(@account)
  end
end
