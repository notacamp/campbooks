module Ai
  # The Campbooks-managed ("Campbooks AI") AI offering: which providers/models the
  # platform runs on a cloud workspace's behalf, and whether it can be offered at all.
  #
  # Single source of truth so onboarding, the setup modal, Settings → AI, and
  # Ai::ProviderSetup#apply_managed all agree. Self-hosted installs never offer it —
  # operators supply their own keys via env (see Ai::ProviderSetup#self_hosted_env_provider?).
  module Platform
    # Both managed text and documents run on Mistral (Paris) so cloud workspace
    # content stays in the EU by default — the GDPR-preferred default. Documents
    # use pixtral-large-latest (vision); text uses mistral-small-latest.
    # NB: requires MISTRAL_API_KEY in the platform env, else managed AI is unavailable.
    MANAGED_TEXT_PROVIDER = "mistral".freeze
    MANAGED_DOC_PROVIDER  = "mistral".freeze

    module_function

    def text_model
      AiConfiguration::DEFAULT_MODEL[MANAGED_TEXT_PROVIDER]
    end

    def doc_model
      AiConfiguration::DOC_DEFAULT_MODEL[MANAGED_DOC_PROVIDER]
    end

    # Can this installation offer "Campbooks AI"? Only on cloud, and only when the
    # platform actually holds the text provider key.
    def available?
      return false if Rails.application.config.self_hosted

      ENV[AiAdapter::PROVIDER_ENV_KEYS[MANAGED_TEXT_PROVIDER]].present?
    end

    # Managed document analysis uses the same Mistral key as text.
    def documents_available?
      available?
    end
  end
end
