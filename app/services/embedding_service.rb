class EmbeddingService
  DEFAULT_MODEL = "text-embedding-3-small"
  DIMENSION = 1536

  EMBEDDING_PROVIDERS = %w[openai gemini].freeze

  def self.embed(text, workspace: nil, model: DEFAULT_MODEL)
    new(workspace).embed(text, model: model)
  end

  def self.embed_batch(texts, workspace: nil, model: DEFAULT_MODEL)
    new(workspace).embed_batch(texts, model: model)
  end

  def initialize(workspace = nil)
    @workspace = workspace
  end

  def embed(text, model: DEFAULT_MODEL)
    return nil if text.blank?

    vectors = embed_batch([ text ], model: model)
    vectors&.first
  end

  def embed_batch(texts, model: DEFAULT_MODEL)
    # Global AI kill-switch (Settings → Data & Privacy) — covers the embedding-only
    # jobs and search paths that bypass ProviderSetup/Configuration.
    return [] if @workspace && !@workspace.ai_processing_enabled?
    return [] if texts.blank?

    valid_texts = texts.select(&:present?)
    return [] if valid_texts.empty?

    primary = find_embedding_adapter
    fallback = env_fallback_adapter

    if primary
      begin
        return primary.embed(valid_texts, model: model)
      rescue => e
        # A stale / invalid stored adapter key (e.g. a 401) shouldn't kill
        # embeddings when a working env key is available — fall back to it.
        raise e unless fallback
        Rails.logger.warn("[EmbeddingService] primary embedding adapter failed (#{e.message}); using env-key fallback")
      end
    end

    return fallback.embed(valid_texts, model: model) if fallback

    Rails.logger.error("[EmbeddingService] No embedding-capable adapter found. Configure OpenAI or Gemini.")
    nil
  end

  private

  def find_embedding_adapter
    # Try workspace's configured adapters first
    if @workspace.present?
      # Prefer OpenAI adapter if available
      openai_adapter = @workspace.ai_adapters.enabled.find_by(provider: "openai")
      return openai_adapter.adapter_instance if openai_adapter

      # Try Gemini as fallback
      gemini_adapter = @workspace.ai_adapters.enabled.find_by(provider: "gemini")
      return gemini_adapter.adapter_instance if gemini_adapter
    end

    if Rails.application.config.self_hosted
      if ENV["OPENAI_API_KEY"].present?
        return Ai::Adapters::Openai.new(api_key: ENV["OPENAI_API_KEY"])
      end

      if ENV["GEMINI_API_KEY"].present?
        return Ai::Adapters::Gemini.new(api_key: ENV["GEMINI_API_KEY"])
      end
    end

    nil
  end

  # Env-key adapter used as a resilience fallback when the workspace's configured
  # embedding adapter is absent or its stored key fails. SELF-HOSTED ONLY: there the
  # key is the operator's own and stays on infrastructure they control. On the
  # managed cloud, falling back to the platform OPENAI_API_KEY/GEMINI_API_KEY would
  # embed user email/document text on a US provider the workspace never configured —
  # a silent data-residency leak — so we fail closed (return nil) instead. Managed
  # cloud workspaces resolve a real OpenAI adapter via #find_embedding_adapter, so
  # this isn't their primary route.
  def env_fallback_adapter
    return nil unless Rails.application.config.self_hosted

    if ENV["OPENAI_API_KEY"].present?
      Ai::Adapters::Openai.new(api_key: ENV["OPENAI_API_KEY"])
    elsif ENV["GEMINI_API_KEY"].present?
      Ai::Adapters::Gemini.new(api_key: ENV["GEMINI_API_KEY"])
    end
  end
end
