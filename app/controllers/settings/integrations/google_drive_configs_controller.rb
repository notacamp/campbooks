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
end
