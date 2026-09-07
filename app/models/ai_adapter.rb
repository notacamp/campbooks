class AiAdapter < ApplicationRecord
  belongs_to :workspace
  has_many :ai_configurations, dependent: :restrict_with_error

  scope :enabled, -> { where(enabled: true) }

  PROVIDERS = AiConfiguration::PROVIDERS
  VISION_PROVIDERS = AiConfiguration::VISION_PROVIDERS

  encrypts :api_key

  validates :name, presence: true, uniqueness: { scope: :workspace_id }
  validates :provider, presence: true, inclusion: { in: PROVIDERS }
  validate :managed_adapter_has_no_key
  validate :managed_adapter_not_self_hosted
  validate :managed_adapter_requires_entitlement
  validate :endpoint_url_must_be_safe

  # TODO: dedupe — identical to AiConfiguration::PROVIDER_ENV_KEYS.
  PROVIDER_ENV_KEYS = {
    "mistral" => "MISTRAL_API_KEY",
    "anthropic" => "ANTHROPIC_API_KEY",
    "openai" => "OPENAI_API_KEY",
    "deepseek" => "DEEPSEEK_API_KEY",
    "gemini" => "GEMINI_API_KEY"
  }.freeze

  def adapter_instance
    key = api_key.presence
    if managed?
      # When an AI gateway is configured, prefer its virtual key over the raw
      # provider key — the gateway holds the real credentials.
      key ||= ENV[Ai::Platform::MANAGED_GATEWAY_KEY_ENV_KEY].presence
      # Fall back to the provider's own platform env key (direct-to-provider path).
      key ||= ENV[PROVIDER_ENV_KEYS[provider]]
    elsif self_hosted?
      key ||= ENV[PROVIDER_ENV_KEYS[provider]]
    end
    Ai::Adapters::Base.for(provider, api_key: key, endpoint_url: endpoint_url)
  end

  def api_key_source
    return "stored"  if api_key.presence
    return "managed" if managed?
    return "env"     if self_hosted? && ENV[PROVIDER_ENV_KEYS[provider]]
    "missing"
  end

  # Would adapter_instance actually have a key to call with? Gates whether this
  # adapter counts as "configured" — a managed adapter whose platform env key has
  # gone missing is not usable, so background jobs stop instead of 401-ing.
  # When an AI gateway is configured, the gateway virtual key counts as a valid
  # credential in addition to the raw provider key.
  def usable?
    return false if managed? && self_hosted? # never a valid combination
    if managed?
      ENV[Ai::Platform::MANAGED_GATEWAY_KEY_ENV_KEY].present? ||
        ENV[PROVIDER_ENV_KEYS[provider]].present?
    else
      api_key.present? || (self_hosted? && ENV[PROVIDER_ENV_KEYS[provider]].present?)
    end
  end

  def in_use?
    ai_configurations.any?
  end

  private

  def managed_adapter_has_no_key
    errors.add(:api_key, :must_be_blank_for_managed) if managed? && api_key.present?
  end

  def managed_adapter_not_self_hosted
    errors.add(:managed, :unavailable_self_hosted) if managed? && self_hosted?
  end

  # Managed "Campbooks AI" must be granted by the workspace's plan. Currently a
  # no-op (every cloud plan grants managed_ai; the billing lever is the deferred
  # managed-AI usage quota), but the gate is wired so a plan that drops managed_ai
  # blocks managed adapters with no further changes. Self-hosted is already
  # handled by managed_adapter_not_self_hosted above.
  def managed_adapter_requires_entitlement
    return unless managed?
    return if self_hosted? || workspace.nil?

    errors.add(:managed, :plan_upgrade_required) unless workspace.entitlements.feature?(:managed_ai)
  end

  # A BYO/self-hosted adapter may point at a custom endpoint; guard it so it can't
  # be aimed at internal infra (cloud metadata, Docker services). The adapter's
  # outbound call carries the API key, so an unguarded URL is SSRF + key exfil.
  # UrlGuard permits local hosts in development (parity with workflow Connections).
  def endpoint_url_must_be_safe
    return if endpoint_url.blank?

    Workflows::UrlGuard.validate!(endpoint_url)
  rescue Workflows::UrlGuard::BlockedError
    errors.add(:endpoint_url, :unsafe)
  end

  def self_hosted?
    Rails.application.config.self_hosted
  end

  def assigned_purposes
    ai_configurations.pluck(:purpose)
  end
end
