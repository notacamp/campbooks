class Settings::GeneralController < Settings::BaseController
  before_action :set_org

  def show
  end

  def update
    @org.settings["workspace_context"] = params[:workspace_context]
    if @org.save
      redirect_to settings_root_path, success: t(".saved")
    else
      render :show, status: :unprocessable_entity
    end
  end

  private

  def set_org
    @org = Current.workspace || current_user&.workspace
  end
end
