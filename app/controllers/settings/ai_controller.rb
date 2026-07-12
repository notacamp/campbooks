class Settings::AiController < Settings::BaseController
  before_action :set_org

  def show
    @adapters = @org.ai_adapters.order(:name)
    @configs = @org.ai_configurations.includes(:ai_adapter).order(:purpose).index_by(&:purpose)
    @managed_available = Ai::Platform.available?
    @using_managed = Ai::ProviderSetup.new(@org).using_managed?

    current_entry = @org.embedding_model_entry
    @embedding_total = SearchChunk.where(workspace: @org).count
    @embedding_stale = SearchChunk.where(workspace: @org).stale_for(current_entry).count
    @embedding_available = EmbeddingService.available_for?(@org, entry: current_entry)
  end

  def embeddings
    key   = params[:embedding_model].to_s
    entry = Ai::EmbeddingModels.find(key)

    unless entry
      return redirect_to settings_ai_path, error: t(".unknown_model")
    end

    unless @org.region_allows?(entry.provider)
      return redirect_to settings_ai_path, error: t(".region_blocked")
    end

    unless EmbeddingService.available_for?(@org, entry: entry)
      provider_name = helpers.human_enum(AiAdapter, :provider, entry.provider)
      return redirect_to settings_ai_path, error: t(".provider_unavailable", provider: provider_name)
    end

    already_selected = @org.embedding_model == entry.key ||
                       (@org.embedding_model.nil? && entry.default?)
    if already_selected
      return redirect_to settings_ai_path, notice: t(".already_selected")
    end

    @org.update!(embedding_model: entry.key)
    Search::WorkspaceReembedJob.perform_later(@org)
    redirect_to settings_ai_path, success: t(".model_changed")
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
