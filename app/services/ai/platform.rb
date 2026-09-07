module Ai
  # The Campbooks-managed ("Campbooks AI") AI offering: which providers/models the
  # platform runs on a cloud workspace's behalf, and whether it can be offered at all.
  #
  # Single source of truth so onboarding, the setup modal, Settings → AI, and
  # Ai::ProviderSetup#apply_managed all agree. Self-hosted installs never offer it —
  # operators supply their own keys via env (see Ai::ProviderSetup#self_hosted_env_provider?).
  #
  # AI gateway (optional): set AI_MANAGED_ENDPOINT to an OpenAI-compatible base URL
  # (e.g. a self-hosted LiteLLM) to route ALL managed AI calls through a central
  # gateway instead of directly to the provider. AI_MANAGED_GATEWAY_KEY optionally
  # overrides the raw provider key with a virtual key accepted by the gateway.
  # When neither is set, behaviour is unchanged: direct to the provider.
  module Platform
    # Both managed text and documents run on Mistral (Paris) so cloud workspace
    # content stays in the EU by default — the GDPR-preferred default. Documents
    # use the multimodal mistral-medium-latest (vision); text uses mistral-small-latest.
    # NB: requires MISTRAL_API_KEY in the platform env, else managed AI is unavailable.
    MANAGED_TEXT_PROVIDER = "mistral".freeze
    MANAGED_DOC_PROVIDER  = "mistral".freeze

    # ENV var names for the optional AI gateway override.
    MANAGED_ENDPOINT_ENV_KEY    = "AI_MANAGED_ENDPOINT".freeze
    MANAGED_GATEWAY_KEY_ENV_KEY = "AI_MANAGED_GATEWAY_KEY".freeze

    module_function

    def text_model
      AiConfiguration::DEFAULT_MODEL[MANAGED_TEXT_PROVIDER]
    end

    def doc_model
      AiConfiguration::DOC_DEFAULT_MODEL[MANAGED_DOC_PROVIDER]
    end

    # Can this installation offer "Campbooks AI"? Only on cloud, and only when the
    # platform actually holds the text provider key (or a gateway key stands in).
    def available?
      return false if Rails.application.config.self_hosted

      ENV[AiAdapter::PROVIDER_ENV_KEYS[MANAGED_TEXT_PROVIDER]].present? ||
        managed_gateway_key.present?
    end

    # Managed document analysis uses the same Mistral key as text.
    def documents_available?
      available?
    end

    # Base URL of the OpenAI-compatible AI gateway, when AI_MANAGED_ENDPOINT is set.
    # Returns nil when unset (direct-to-provider path).
    def managed_gateway_endpoint
      ENV[MANAGED_ENDPOINT_ENV_KEY].presence
    end

    # Virtual/gateway API key to send to the gateway instead of the raw provider key,
    # when AI_MANAGED_GATEWAY_KEY is set. Returns nil when unset.
    def managed_gateway_key
      ENV[MANAGED_GATEWAY_KEY_ENV_KEY].presence
    end
  end
end
