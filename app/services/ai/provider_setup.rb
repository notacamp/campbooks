module Ai
  # Maps the user-facing "two providers" model onto the underlying per-purpose
  # AiConfiguration records:
  #
  #   • Text provider     → every TEXT_PURPOSE (email triage, replies, chat…)
  #   • Document provider → DOCUMENT_PURPOSES (PDF/image analysis; vision only)
  #
  # Onboarding and the setup modal both call apply_text/apply_documents to upsert
  # one adapter per role and point that role's purposes at it. The role adapter is
  # reused in place when it already exists, so re-entering setup updates the key
  # without spawning duplicate adapters. The granular per-purpose matrix still
  # lives in Settings → AI for power users.
  class ProviderSetup
    TEXT_ADAPTER_NAME = "Text AI provider".freeze
    VISION_ADAPTER_NAME = "Document AI provider".freeze
    MANAGED_TEXT_ADAPTER_NAME = "Campbooks AI — Text".freeze
    MANAGED_VISION_ADAPTER_NAME = "Campbooks AI — Documents".freeze

    # The capabilities a feature can require. Each AI surface maps to exactly one:
    #   • :text       — chat/triage/replies/reminders (self-hosted-only legacy Anthropic fallback)
    #   • :documents  — PDF/image vision analysis (no fallback; DB config required)
    #   • :embeddings — semantic search & tag classification; provider follows the
    #                   workspace's embedding model selection (Settings → AI).
    #                   Mistral gives EU-residency workspaces a compliant option.
    CAPABILITIES = %i[text documents embeddings].freeze

    # Convenience: "will <capability> actually work for this workspace right now?"
    def self.available?(workspace, capability)
      new(workspace).available?(capability)
    end

    # Strict counterpart of .available? — see the instance #configured?.
    def self.configured?(workspace, capability)
      new(workspace).configured?(capability)
    end

    # Provision managed "Campbooks AI" as the DEFAULT for a brand-new cloud
    # workspace, so every signup has working text + document AI from the first
    # sign-in — even when the user skips the onboarding AI step (the gap that
    # left existing beta customers with no AI at all). No-op on self-hosted or
    # when the platform isn't holding its keys (Ai::Platform.available?).
    # Best-effort: a provider hiccup is logged/reported but never blocks
    # signup or onboarding. Returns true when managed AI was applied.
    def self.apply_managed_default(workspace)
      return false unless workspace && Ai::Platform.available?

      new(workspace).apply_managed
      true
    rescue => e
      Rails.logger.error("[ai] managed-default provisioning failed for workspace #{workspace&.id}: #{e.class}: #{e.message}")
      Rails.error.report(e, handled: true, context: { workspace_id: workspace&.id })
      false
    end

    def initialize(workspace)
      @workspace = workspace
    end

    # Runtime gate for the "set up AI" prompts and interactive, user-invoked AI
    # (Scout chat, "draft a reply"): true when the capability would genuinely
    # work, counting the shared platform env keys as a convenience.
    def available?(capability)
      return false unless @workspace&.ai_processing_enabled?

      case capability.to_sym
      when :text       then text_available?
      when :documents  then documents_available?
      when :embeddings then embeddings_available?
      else false
      end
    end

    # Stricter than #available?: "has THIS workspace opted into AI by setting up a
    # provider?" Excludes the shared hosted-platform env keys that #available?
    # counts. Gates AUTOMATIC background processing — auto-triage, classification,
    # reminder extraction, embeddings — so a freshly-synced inbox isn't analysed
    # on the platform's keys before the user opts in. (Self-hosted env keys are
    # the operator's own, so they still count.)
    def configured?(capability)
      return false unless @workspace&.ai_processing_enabled?

      case capability.to_sym
      when :text       then text_configured? || self_hosted_env_provider?
      when :documents  then documents_configured?
      when :embeddings then embeddings_configured?
      else false
      end
    end

    # Upsert the text adapter and wire every text purpose to it. A blank api_key
    # keeps the stored one (password fields submit blank on re-entry).
    def apply_text(provider:, api_key: nil, endpoint_url: nil)
      adapter = upsert_role_adapter(AiConfiguration::TEXT_PURPOSES, TEXT_ADAPTER_NAME,
                                    provider: provider, api_key: api_key, endpoint_url: endpoint_url)
      assign_purposes(adapter, AiConfiguration::TEXT_PURPOSES)
      adapter
    end

    # Upsert the document/image adapter and wire document_analysis to it. No-op
    # unless the provider is vision-capable (DeepSeek can't read images).
    def apply_documents(provider:, api_key: nil, endpoint_url: nil)
      return nil unless AiConfiguration::VISION_PROVIDERS.include?(provider)

      adapter = upsert_role_adapter(AiConfiguration::DOCUMENT_PURPOSES, VISION_ADAPTER_NAME,
                                    provider: provider, api_key: api_key, endpoint_url: endpoint_url)
      assign_purposes(adapter, AiConfiguration::DOCUMENT_PURPOSES)
      adapter
    end

    # Wire the workspace to the Campbooks-managed adapters (platform keys, no stored
    # key). Mirrors apply_text/apply_documents and is idempotent — re-running re-enables
    # and re-points without spawning duplicates. Cloud only: on self-hosted the
    # operator's own env keys already serve every workspace, so managed has no meaning.
    def apply_managed
      raise "managed AI is unavailable on self-hosted installations" if Rails.application.config.self_hosted

      text = upsert_managed_adapter(MANAGED_TEXT_ADAPTER_NAME, provider: Ai::Platform::MANAGED_TEXT_PROVIDER)
      assign_purposes(text, AiConfiguration::TEXT_PURPOSES)

      # Managed document analysis runs on Mistral (EU), same as text.
      if Ai::Platform.documents_available? && @workspace.region_allows?(Ai::Platform::MANAGED_DOC_PROVIDER)
        docs = upsert_managed_adapter(MANAGED_VISION_ADAPTER_NAME, provider: Ai::Platform::MANAGED_DOC_PROVIDER)
        assign_purposes(docs, AiConfiguration::DOCUMENT_PURPOSES)
      end

      # Managed text/docs run on Mistral (EU). Pin embeddings to the Mistral
      # entry too so new managed workspaces embed without needing an OpenAI key.
      # Only set when the workspace hasn't already chosen an explicit model — never
      # override a deliberate selection.
      if @workspace.embedding_model.nil?
        mistral_entry = Ai::EmbeddingModels.find("mistral/mistral-embed")
        @workspace.update!(embedding_model: mistral_entry.key)
      end

      text
    end

    # Text AI is usable: a text purpose is configured, or (self-hosted) the
    # operator's own env key stands in via the legacy Anthropic fallback. The bare
    # shared ANTHROPIC_API_KEY is no longer counted on the cloud — the legacy
    # fallback only fires on self-hosted now (Ai::LegacyFallback) — so "available"
    # tracks a real configured/managed provider rather than a silent platform key.
    # This is the gate for "AI is set up" — see SetupStatus.
    def text_available?
      text_configured? || self_hosted_env_provider?
    end

    def text_configured?
      @workspace.ai_configurations
                .where(purpose: AiConfiguration::TEXT_PURPOSES, enabled: true)
                .includes(:ai_adapter).any? { |c| c.ai_adapter.usable? }
    end

    # Document analysis fails fast without a DB config (no env/legacy fallback),
    # so availability is exactly "a document purpose is configured".
    def documents_available?
      documents_configured?
    end

    # Document analysis fails fast without a DB config (no env fallback). A wired
    # adapter only counts if it's actually usable — i.e. its managed/self-hosted env
    # key is present (a stored BYO key always is).
    def documents_configured?
      @workspace.ai_configurations
                .where(purpose: AiConfiguration::DOCUMENT_PURPOSES, enabled: true)
                .includes(:ai_adapter).any? { |c| c.ai_adapter.usable? }
    end

    # Embeddings are available when EmbeddingService can resolve a usable adapter
    # for the workspace's current embedding model entry (respects region policy,
    # provider-specific adapter, and self-hosted env-key fallback). Delegates
    # entirely to EmbeddingService so the two stay in sync.
    def embeddings_available?
      EmbeddingService.available_for?(@workspace)
    end

    # Strict counterpart of #embeddings_available? used to gate AUTOMATIC
    # background embedding (auto-classify, re-embed sweeper). Delegates to
    # EmbeddingService.available_for? — which already fails closed on the cloud
    # (no silent platform-key embedding before the workspace opts in) — so the
    # two are equivalent in their gating semantics.
    def embeddings_configured?
      EmbeddingService.available_for?(@workspace)
    end

    # Current provider for each role, for prefilling the setup forms (nil if the
    # role isn't configured yet).
    def text_provider
      role_adapter(AiConfiguration::TEXT_PURPOSES)&.provider
    end

    def document_provider
      role_adapter(AiConfiguration::DOCUMENT_PURPOSES)&.provider
    end

    # True when the workspace's text role is driven by a managed ("Campbooks AI")
    # adapter — lets onboarding/settings preselect the right mode.
    def using_managed?
      role_adapter(AiConfiguration::TEXT_PURPOSES)&.managed? || false
    end

    private

    def upsert_role_adapter(purposes, default_name, provider:, api_key:, endpoint_url:)
      # Reuse the current role adapter in place — unless it's a managed one. A managed
      # adapter can't hold a key (validation), so a switch to BYO must land on the
      # dedicated named row instead, leaving the managed adapter untouched.
      current = role_adapter(purposes)
      adapter = (current unless current&.managed?) ||
                @workspace.ai_adapters.find_or_initialize_by(name: default_name)
      adapter.provider = provider
      adapter.managed = false
      adapter.enabled = true
      adapter.api_key = api_key if api_key.present?
      adapter.endpoint_url = endpoint_url if endpoint_url.present?
      adapter.save!
      adapter
    end

    # Like upsert_role_adapter, but flags the adapter managed and never stores a key —
    # adapter_instance resolves it from the platform env at call time.
    def upsert_managed_adapter(name, provider:)
      adapter = @workspace.ai_adapters.find_or_initialize_by(name: name)
      adapter.provider = provider
      adapter.managed = true
      adapter.enabled = true
      adapter.api_key = nil
      adapter.save!
      adapter
    end

    def assign_purposes(adapter, purposes)
      purposes.each do |purpose|
        config = @workspace.ai_configurations.find_or_initialize_by(purpose: purpose)
        config.ai_adapter = adapter
        config.enabled = true
        config.model = resolved_model_for(adapter, config.model, purpose)
        config.max_tokens ||= 1000
        config.temperature ||= 0.0
        config.save!
      end
    end

    # Pick the model for a (purpose, adapter) pair. A managed adapter always runs
    # the platform-chosen default for its provider. Document analysis uses the
    # DOC_DEFAULT_MODEL (vision-capable); everything else uses DEFAULT_MODEL.
    # A BYO adapter keeps the user's explicit model ONLY when it's valid for the
    # adapter's provider; otherwise it falls back to that provider's default — so
    # switching a role to a new provider (e.g. document analysis OpenAI → Anthropic)
    # never leaves a stale model like "gpt-4o-mini" on a Claude adapter.
    def resolved_model_for(adapter, current_model, purpose = nil)
      valid = AiConfiguration::MODELS[adapter.provider] || []
      if !adapter.managed? && current_model.present? && valid.include?(current_model)
        current_model
      elsif purpose == "document_analysis"
        AiConfiguration::DOC_DEFAULT_MODEL[adapter.provider] || AiConfiguration::DEFAULT_MODEL[adapter.provider] || valid.first || "gpt-4o-mini"
      else
        AiConfiguration::DEFAULT_MODEL[adapter.provider] || valid.first || "gpt-4o-mini"
      end
    end

    # The adapter currently driving a role = whatever its purposes point at.
    def role_adapter(purposes)
      @workspace.ai_configurations
                .where(purpose: purposes).includes(:ai_adapter)
                .order(:purpose).first&.ai_adapter
    end

    def self_hosted_env_provider?
      return false unless Rails.application.config.self_hosted

      AiConfiguration::PROVIDER_ENV_KEYS.values.any? { |key| ENV[key].present? }
    end
  end
end
