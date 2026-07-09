class Settings::GeneralController < Settings::BaseController
  before_action :set_org

  def show
  end

  def update
    @org.settings["workspace_context"] = params[:workspace_context]

    # NIF (company VAT number): strip whitespace; empty string → remove key.
    nif = params[:company_nif].to_s.strip
    if nif.present?
      @org.settings["company_nif"] = nif
    else
      @org.settings.delete("company_nif")
    end

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
