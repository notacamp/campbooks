class Settings::AiController < Settings::BaseController
  before_action :set_org

  def show
    @adapters = @org.ai_adapters.order(:name)
    @configs = @org.ai_configurations.includes(:ai_adapter).order(:purpose).index_by(&:purpose)
    @managed_available = Ai::Platform.available?
    @using_managed = Ai::ProviderSetup.new(@org).using_managed?
  end

  # Flip the workspace between Campbooks-managed AI and bring-your-own keys.
  # Non-destructive: the inactive side's adapters are disabled, never deleted, so a
  # stored BYO key survives a round-trip through managed.
  def switch_mode
    setup = Ai::ProviderSetup.new(@org)

    case params[:mode]
    when "managed"
      if Ai::Platform.available? && !self_hosted?
        disable_byo_role_adapters
        setup.apply_managed
        redirect_to settings_ai_path, success: t(".switched_to_managed")
      else
        redirect_to settings_ai_path, error: t(".managed_unavailable")
      end
    when "byo"
      @org.ai_adapters.where(managed: true).update_all(enabled: false)
      redirect_to settings_ai_path, success: t(".switched_to_byo")
    else
      redirect_to settings_ai_path, error: t(".invalid_mode")
    end
  end

  private

  def set_org
    @org = Current.workspace || current_user&.workspace
  end

  # Disable the (non-managed) adapters currently wired to the text/document roles, so
  # switching to managed leaves them dormant rather than competing for the purposes.
  def disable_byo_role_adapters
    ids = @org.ai_configurations
              .where(purpose: AiConfiguration::TEXT_PURPOSES + AiConfiguration::DOCUMENT_PURPOSES)
              .joins(:ai_adapter).where(ai_adapters: { managed: false })
              .pluck(:ai_adapter_id)
    @org.ai_adapters.where(id: ids).update_all(enabled: false)
  end

  def current_section
    "ai"
  end
end
