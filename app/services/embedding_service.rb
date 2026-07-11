# frozen_string_literal: true

class EmbeddingService
  # Legacy constant kept so any existing callers that reference DEFAULT_MODEL
  # continue to compile without changes.
  DEFAULT_MODEL = "text-embedding-3-small"

  # Providers that can produce embeddings. Cross-provider fallback is intentionally
  # absent: a workspace configured for mistral-embed (1024 dims) must NEVER silently
  # fall back to openai/text-embedding-3-small (1536 dims) — the vectors would live
  # in incompatible spaces and produce nonsense similarity scores.
  EMBEDDING_PROVIDERS = %w[openai gemini mistral].freeze

  # Convenience class methods — entry resolves from workspace when omitted.
  def self.embed(text, workspace: nil, entry: nil)
    new(workspace, entry: entry).embed(text)
  end

  def self.embed_batch(texts, workspace: nil, entry: nil)
    new(workspace, entry: entry).embed_batch(texts)
  end

  # True iff adapter resolution would succeed for this workspace + entry combo.
  # Used by background jobs to gate embedding work without attempting a real call.
  def self.available_for?(workspace, entry: nil)
    new(workspace, entry: entry).send(:find_embedding_adapter).present?
  end

  # +entry+ pins the catalog entry; when nil the workspace's configured entry
  # (or the DEFAULT) is used.
  def initialize(workspace = nil, entry: nil)
    @workspace = workspace
    @entry = entry || workspace&.embedding_model_entry || Ai::EmbeddingModels::DEFAULT
  end

  def embed(text)
    return nil if text.blank?

    vectors = embed_batch([ text ])
    vectors&.first
  end

  def embed_batch(texts)
    # Global AI kill-switch (Settings -> Data & Privacy) — covers the embedding-only
    # jobs and search paths that bypass ProviderSetup/Configuration.
    return [] if @workspace && !@workspace.ai_processing_enabled?
    return [] if texts.blank?

    valid_texts = texts.select(&:present?)
    return [] if valid_texts.empty?

    # Truncate each input to this entry's character budget before hitting the API.
    capped_texts = valid_texts.map { |t| t.to_s[0, @entry.max_input_chars] }

    primary = find_embedding_adapter
    fallback = env_fallback_adapter

    if primary
      begin
        raw = primary.embed(capped_texts, model: @entry.model, dimensions: @entry.request_dimensions)
        return normalize_vectors(raw)
      rescue => e
        # A stale/invalid stored adapter key (e.g. a 401) shouldn't kill embeddings
        # when the SAME provider's env key is available — fall back to it.
        # NOTE: we fall back only within the same provider; a different provider would
        # produce vectors in a different model space, silently breaking similarity scores.
        raise e unless fallback
        Rails.logger.warn("[EmbeddingService] primary embedding adapter failed (#{e.message}); using env-key fallback")
      end
    end

    if fallback
      raw = fallback.embed(capped_texts, model: @entry.model, dimensions: @entry.request_dimensions)
      return normalize_vectors(raw)
    end

    Rails.logger.error("[EmbeddingService] No embedding-capable adapter found for provider #{@entry.provider}. " \
                       "Configure #{@entry.provider.capitalize} or set #{provider_env_key} (self-hosted).")
    nil
  end

  private

  # Provider-directed adapter resolution:
  #   1. Region gate: workspace.region_allows?(provider) must pass.
  #   2. Workspace adapters for this provider — prefer BYO (non-managed) over managed.
  #   3. Self-hosted only: env key for this provider.
  def find_embedding_adapter
    provider = @entry.provider

    # EU data-residency gate — Mistral (EU) is now a valid option for EU workspaces,
    # unlike OpenAI/Gemini (US). If the workspace's region policy blocks this
    # provider, embedding is paused rather than silently switching providers.
    return nil if @workspace && !@workspace.region_allows?(provider)

    if @workspace.present?
      # Prefer BYO (stored key, managed: false) over managed adapters so user-supplied
      # keys win over any platform key, then fall through to env fallback if none usable.
      adapter_row = @workspace.ai_adapters.enabled
                              .where(provider: provider)
                              .order(managed: :asc)
                              .detect(&:usable?)
      return adapter_row.adapter_instance if adapter_row
    end

    # Self-hosted only: use the operator's own env key for this specific provider.
    # On the managed cloud, falling back to a platform env key would embed user text
    # on a provider the workspace never configured — a silent data-residency leak —
    # so we fail closed (return nil) for non-self-hosted.
    if Rails.application.config.self_hosted
      env_key = ENV[provider_env_key]
      return build_env_adapter(provider, env_key) if env_key.present?
    end

    nil
  end

  # Env-key adapter used as a resilience fallback when the workspace's configured
  # adapter's stored key is stale/invalid. SELF-HOSTED ONLY for the same reason as
  # #find_embedding_adapter: on the managed cloud failing closed is safer than
  # silently embedding with the platform's key. Falls back only to the SAME
  # provider — crossing providers would swap model spaces (latent bug in the old code).
  def env_fallback_adapter
    return nil unless Rails.application.config.self_hosted
    return nil if @workspace && !@workspace.region_allows?(@entry.provider)

    env_key = ENV[provider_env_key]
    build_env_adapter(@entry.provider, env_key) if env_key.present?
  end

  def provider_env_key
    AiAdapter::PROVIDER_ENV_KEYS[@entry.provider]
  end

  def build_env_adapter(provider, api_key)
    Ai::Adapters::Base.for(provider, api_key: api_key)
  end

  # L2-normalise each vector so that dot-product == cosine similarity.
  # Gemini returns truncated-dimension vectors that may not be unit-norm; the
  # downstream weighted-averaging in search assumes unit norms. Zero vectors are
  # left as-is (they carry no directional information and dividing by zero is wrong).
  def normalize_vectors(vectors)
    return vectors unless vectors.is_a?(Array)

    vectors.map do |vec|
      next vec unless vec.is_a?(Array) && vec.any?

      norm = Math.sqrt(vec.sum { |v| v * v })
      next vec if norm.zero?

      vec.map { |v| v / norm }
    end
  end
end
