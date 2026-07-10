class Settings::GeneralController < Settings::BaseController
  before_action :set_org

  def show
  end

  def update
    # Both fields are guarded with params.key? so a request that doesn't carry
    # a field never clears it — a partial post (another form, API client, or a
    # form/controller param-shape drift) must not wipe stored settings. This
    # exact drift wiped workspace_context in prod (2026-07-09, v0.19.2).
    if params.key?(:workspace_context)
      @org.settings["workspace_context"] = params[:workspace_context]
    end

    # NIF (company VAT number): strip whitespace; empty string → remove key.
    # Guard with params.key? so other forms posting to this action don't wipe
    # the NIF when they don't include the field at all.
    if params.key?(:company_nif)
      nif = params[:company_nif].to_s.strip
      if nif.present?
        @org.settings["company_nif"] = nif
      else
        @org.settings.delete("company_nif")
      end
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
