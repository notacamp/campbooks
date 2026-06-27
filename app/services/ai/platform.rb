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
    # Documents need vision. Anthropic reads PDFs natively (type: "document" blocks),
    # so no ImageMagick/ghostscript dependency — unlike OpenAI, which requires a
    # fragile PDF→image conversion step. Falls back to OpenAI when the Anthropic key
    # isn't provisioned, so the managed offering degrades gracefully.
    # NB: requires MISTRAL_API_KEY in the platform env, else managed AI is unavailable.
    MANAGED_TEXT_PROVIDER = "mistral".freeze
    MANAGED_DOC_PROVIDER  = "anthropic".freeze
    DOC_FALLBACK_PROVIDER = "openai".freeze

    module_function

    def text_model
      AiConfiguration::DEFAULT_MODEL[MANAGED_TEXT_PROVIDER]
    end

    def doc_model
      AiConfiguration::DEFAULT_MODEL[resolved_doc_provider]
    end

    # Can this installation offer "Campbooks AI"? Only on cloud, and only when the
    # platform actually holds the text provider key.
    def available?
      return false if Rails.application.config.self_hosted

      ENV[AiAdapter::PROVIDER_ENV_KEYS[MANAGED_TEXT_PROVIDER]].present?
    end

    # Managed document analysis additionally needs a vision provider key. Prefers
    # Anthropic (native PDF), but degrades to OpenAI when only the OpenAI key is set.
    def documents_available?
      return false unless available?

      ENV[AiAdapter::PROVIDER_ENV_KEYS[MANAGED_DOC_PROVIDER]].present? ||
        ENV[AiAdapter::PROVIDER_ENV_KEYS[DOC_FALLBACK_PROVIDER]].present?
    end

    # The best available managed document provider — Anthropic if its key is
    # provisioned, otherwise OpenAI. Returns nil when neither key is set.
    def resolved_doc_provider
      if ENV[AiAdapter::PROVIDER_ENV_KEYS[MANAGED_DOC_PROVIDER]].present?
        MANAGED_DOC_PROVIDER
      elsif ENV[AiAdapter::PROVIDER_ENV_KEYS[DOC_FALLBACK_PROVIDER]].present?
        DOC_FALLBACK_PROVIDER
      end
    end
  end
end
