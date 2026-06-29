class AiConfiguration < ApplicationRecord
  belongs_to :workspace
  belongs_to :ai_adapter

  PROVIDERS = %w[mistral anthropic openai deepseek gemini].freeze
  VISION_PROVIDERS = %w[openai anthropic gemini mistral].freeze # deepseek text-only; Mistral pixtral now GA

  # Data residency of each provider's API, surfaced in the AI settings UI for GDPR
  # transparency. Mistral (Paris) keeps processing in the EU — the preferred
  # default; the rest are international transfers, and DeepSeek (China) has no EU
  # adequacy decision.
  PROVIDER_REGIONS = {
    "mistral"   => "EU",
    "anthropic" => "US",
    "openai"    => "US",
    "gemini"    => "US",
    "deepseek"  => "China"
  }.freeze

  PURPOSES = %w[
    document_analysis
    email_classification
    email_analysis
    reminder_extraction
    document_template_generation
    email_template_generation
    email_chat
    draft_reply
    global_chat
    compose_chat
  ].freeze

  # The seven granular purposes collapse into two user-facing provider "roles":
  # a text model (anything) and a vision model (document/image analysis). Setup
  # configures the two roles; each role fans out to the purposes below. The text
  # list is ordered by suitability as a general-chat fallback (see
  # Ai::Configuration.for_any). DOCUMENT_PURPOSES must use a VISION_PROVIDER.
  TEXT_PURPOSES = %w[
    global_chat
    email_chat
    compose_chat
    document_template_generation
    email_template_generation
    email_analysis
    email_classification
    draft_reply
    reminder_extraction
  ].freeze
  DOCUMENT_PURPOSES = %w[document_analysis].freeze

  PROVIDER_ENV_KEYS = {
    "mistral"   => "MISTRAL_API_KEY",
    "anthropic" => "ANTHROPIC_API_KEY",
    "openai"    => "OPENAI_API_KEY",
    "deepseek"  => "DEEPSEEK_API_KEY",
    "gemini"    => "GEMINI_API_KEY"
  }.freeze

  MODELS = {
    "mistral"   => %w[mistral-large-latest mistral-medium-latest mistral-small-latest ministral-8b-latest pixtral-large-latest],
    "openai"    => %w[gpt-4o gpt-4o-mini gpt-4-turbo gpt-4 gpt-3.5-turbo o3-mini o4-mini],
    "anthropic" => %w[claude-sonnet-4-6 claude-haiku-4-5 claude-opus-4-7 claude-sonnet-4-5],
    "deepseek"  => %w[deepseek-v4-pro deepseek-chat deepseek-reasoner],
    "gemini"    => %w[gemini-2.5-flash gemini-2.5-pro gemini-2.0-flash gemini-1.5-pro gemini-1.5-flash]
  }.freeze

  DEFAULT_MODEL = {
    "mistral"   => "mistral-small-latest",
    "openai"    => "gpt-4o-mini",
    "anthropic" => "claude-sonnet-4-6",
    "deepseek"  => "deepseek-v4-pro",
    "gemini"    => "gemini-2.0-flash"
  }.freeze

  # Document analysis needs a vision-capable model, which may differ from the
  # provider's general-purpose text default (e.g. Mistral uses pixtral for docs
  # vs mistral-small for text). Falls back to DEFAULT_MODEL when absent.
  DOC_DEFAULT_MODEL = {
    "mistral"   => "pixtral-large-latest",
    "openai"    => "gpt-4o-mini",
    "anthropic" => "claude-sonnet-4-6",
    "gemini"    => "gemini-2.0-flash"
  }.freeze

  validates :purpose, presence: true, uniqueness: { scope: :workspace_id }, inclusion: { in: PURPOSES }
  validate :document_analysis_requires_vision_provider, if: -> { purpose == "document_analysis" }

  def document_analysis_requires_vision_provider
    if ai_adapter && !VISION_PROVIDERS.include?(ai_adapter.provider)
      errors.add(:ai_adapter, :not_vision_capable)
    end
  end
  validates :ai_adapter_id, presence: true
  validates :model, presence: true
  validate :model_permitted_by_entitlements
  validates :max_tokens, presence: true, numericality: { greater_than: 0 }
  validates :temperature, presence: true, numericality: { greater_than_or_equal_to: 0.0, less_than_or_equal_to: 2.0 }
  validates :system_prompt, length: { maximum: 2000, message: :too_long }, allow_blank: true

  delegate :provider, :adapter_instance, :api_key_source, to: :ai_adapter

  def purpose_label
    purpose.titleize
  end

  def models_for_provider
    MODELS[provider] || []
  end

  def default_model
    DEFAULT_MODEL[provider] || MODELS[provider]&.first
  end

  private

  # Gate model selection by the workspace's ai_model_access tier. A no-op today
  # (all tiers unrestricted in Entitlements::AiModelTiers); narrowing a tier there
  # starts enforcing here with no further changes.
  def model_permitted_by_entitlements
    return if workspace.nil? || model.blank?

    allowed = Entitlements::AiModelTiers.models_for(
      workspace.entitlements.config(:ai_model_access, :tier)
    )
    return if allowed.nil? || allowed.include?(model)

    errors.add(:model, :not_permitted_on_plan)
  end
end
