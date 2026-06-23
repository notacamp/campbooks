module Ai
  # The Campbooks-managed ("Campbooks AI") AI offering: which providers/models the
  # platform runs on a cloud workspace's behalf, and whether it can be offered at all.
  #
  # Single source of truth so onboarding, the setup modal, Settings → AI, and
  # Ai::ProviderSetup#apply_managed all agree. Self-hosted installs never offer it —
  # operators supply their own keys via env (see Ai::ProviderSetup#self_hosted_env_provider?).
  module Platform
    # Managed text runs on Mistral (Paris) so a cloud workspace's email/chat content
    # stays in the EU by default — the GDPR-preferred default (was DeepSeek/China).
    # Documents need vision, so they still run on OpenAI (Mistral pixtral/vision for
    # documents is a tracked follow-up). Both reuse the BYO-setup default models.
    # NB: requires MISTRAL_API_KEY in the platform env, else managed AI is unavailable.
    MANAGED_TEXT_PROVIDER = "mistral".freeze
    MANAGED_DOC_PROVIDER  = "openai".freeze

    module_function

    def text_model
      AiConfiguration::DEFAULT_MODEL[MANAGED_TEXT_PROVIDER]
    end

    def doc_model
      AiConfiguration::DEFAULT_MODEL[MANAGED_DOC_PROVIDER]
    end

    # Can this installation offer "Campbooks AI"? Only on cloud, and only when the
    # platform actually holds the text provider key.
    def available?
      return false if Rails.application.config.self_hosted

      ENV[AiAdapter::PROVIDER_ENV_KEYS[MANAGED_TEXT_PROVIDER]].present?
    end

    # Managed document analysis additionally needs the vision provider key.
    def documents_available?
      available? && ENV[AiAdapter::PROVIDER_ENV_KEYS[MANAGED_DOC_PROVIDER]].present?
    end
  end
end
