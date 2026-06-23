class Settings::AiAdaptersController < Settings::BaseController
  before_action :set_adapter, only: [ :update, :destroy ]

  def create
    @adapter = current_user.workspace.ai_adapters.new(adapter_params)
    if @adapter.save
      redirect_to settings_ai_path, success: t(".created", name: @adapter.name)
    else
      redirect_to settings_ai_path, error: t(".failed", errors: @adapter.errors.full_messages.to_sentence)
    end
  end

  def update
    if @adapter.update(adapter_params)
      if adapter_params[:api_key].present?
        redirect_to settings_ai_path, success: t(".updated", name: @adapter.name)
      else
        redirect_to settings_ai_path, success: t(".updated_env_key", name: @adapter.name)
      end
    else
      redirect_to settings_ai_path, error: t(".failed", errors: @adapter.errors.full_messages.to_sentence)
    end
  end

  def destroy
    if @adapter.ai_configurations.any?
      redirect_to settings_ai_path, error: t(".in_use", name: @adapter.name)
    else
      @adapter.destroy!
      redirect_to settings_ai_path, success: t(".deleted", name: @adapter.name)
    end
  end

  private

  def set_adapter
    @adapter = current_user.workspace.ai_adapters.find(params[:id])
    # Managed ("Campbooks AI") adapters are owned by the platform — switch modes in
    # Settings → AI instead of editing/deleting them here.
    if @adapter.managed?
      redirect_to settings_ai_path, error: t("settings.ai_adapters.managed_not_editable")
    end
  end

  def adapter_params
    params.require(:ai_adapter).permit(
      :name, :provider, :api_key, :endpoint_url, :enabled
    )
  end
end
