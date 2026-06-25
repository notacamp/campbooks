module Ai
  # Records which provider / model / data region produced an AI output, persisted as
  # a small jsonb hash (agent_messages.ai_provenance, email_messages.ai_provenance,
  # or documents.ai_extraction_data["_provenance"]) so the UI can show
  # "Processed by <provider> · <region>" in context. {} when not AI-generated.
  module Provenance
    module_function

    # The provenance for a purpose, resolving the workspace's config the same way
    # the AI services do: the managed/BYO adapter via Ai::Configuration.for, else
    # the legacy self-hosted Anthropic fallback when (and only when) it's allowed.
    # {} when no provider would actually run — AI off, or unconfigured on the cloud
    # — matching the services, which produce no output (so nothing to attribute).
    def for_purpose(purpose, legacy_model: nil)
      config = Ai::Configuration.for(purpose)
      return from_config(config) if config
      return legacy(legacy_model) if Ai::LegacyFallback.allowed?

      {}
    end

    # From a resolved Ai::Configuration.for(...) hash (managed or BYO).
    def from_config(config)
      return {} unless config

      build(config[:provider], config[:model])
    end

    # The legacy single-provider fallback (Ai::LegacyFallback) always calls Anthropic.
    def legacy(model)
      build("anthropic", model)
    end

    def build(provider, model)
      return {} if provider.blank?

      { "provider" => provider, "model" => model, "region" => AiConfiguration::PROVIDER_REGIONS[provider] }
    end
  end
end
