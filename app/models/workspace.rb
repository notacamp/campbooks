class Workspace < ApplicationRecord
  has_many :users, dependent: :restrict_with_error
  has_many :email_accounts, dependent: :restrict_with_error
  has_many :calendar_accounts, dependent: :restrict_with_error
  has_many :documents, dependent: :restrict_with_error
  has_many :authored_documents, dependent: :restrict_with_error
  has_many :document_types, dependent: :restrict_with_error
  has_many :tags, dependent: :restrict_with_error
  has_many :ai_adapters, dependent: :destroy
  has_many :ai_configurations, dependent: :destroy
  has_many :notion_integrations, dependent: :destroy
  has_many :google_drive_accounts, dependent: :destroy
  has_many :zoho_drive_accounts, dependent: :destroy
  has_many :contacts, dependent: :restrict_with_error
  has_many :people, dependent: :restrict_with_error
  has_many :organizations, dependent: :restrict_with_error
  has_many :agent_threads, dependent: :destroy
  has_many :exports, dependent: :destroy
  has_many :workflows, dependent: :destroy
  has_many :connections, dependent: :destroy
  has_many :feed_items, dependent: :delete_all
  has_many :events, dependent: :delete_all
  has_many :bug_reports, dependent: :destroy
  has_many :reminders, dependent: :destroy
  has_many :mail_folders, dependent: :destroy

  has_many :invitations, dependent: :destroy

  INBOX_FILTER_STRATEGIES = %w[blacklist whitelist].freeze

  validates :name, presence: true
  validates :slug, presence: true, uniqueness: true
  validates :plan, presence: true, inclusion: { in: ->(_) { Entitlements::Catalog.plan_names } }
  validate :inbox_filter_strategy_valid
  validate :entitlement_overrides_valid

  before_validation :generate_slug, on: :create

  def company?
    workspace_type == "company"
  end

  def individual?
    workspace_type == "individual"
  end

  def setting(key, default = nil)
    settings&.fetch(key.to_s, default)
  end

  # Data-residency policy: may this workspace use an AI provider in the given
  # provider's region? True when no policy is set, or the provider's region
  # (AiConfiguration::PROVIDER_REGIONS) matches the required one. The single gate
  # behind EU-residency enforcement (Ai::Configuration.for, EmbeddingService,
  # managed provisioning all consult it).
  def region_allows?(provider)
    return true if required_data_region.blank?

    AiConfiguration::PROVIDER_REGIONS[provider.to_s] == required_data_region
  end

  # Effective feature entitlements for this workspace. Self-hosted installs are
  # always unlimited (NullResolver); cloud workspaces resolve their plan from the
  # catalog deep-merged with entitlement_overrides (Entitlements::Resolver).
  def entitlements
    if Rails.application.config.self_hosted
      Entitlements::NullResolver.new
    else
      Entitlements::Resolver.new(self)
    end
  end

  def company_name
    setting("company_name", name)
  end

  def company_nif
    setting("company_nif")
  end

  def app_name
    setting("app_name", "Campbooks")
  end

  def workspace_context
    setting("workspace_context")
  end

  def default_currency
    setting("default_currency", "EUR")
  end

  # How unknown senders are treated. "blacklist" (default) = everything reaches the
  # inbox and senders can be blocked one by one. "whitelist" = only allowed/starred
  # senders reach the inbox; unknown senders wait in Skim's Pending bucket.
  def inbox_filter_strategy
    setting("inbox_filter_strategy", "blacklist")
  end

  def whitelist_mode?
    inbox_filter_strategy == "whitelist"
  end

  private

  def inbox_filter_strategy_valid
    value = settings && settings["inbox_filter_strategy"]
    return if value.blank? || INBOX_FILTER_STRATEGIES.include?(value)

    errors.add(:settings, :invalid_inbox_filter_strategy)
  end

  # Reject malformed entitlement overrides (unknown feature key, wrong value
  # type) against the composed JSON Schema. Mirrors inbox_filter_strategy_valid.
  def entitlement_overrides_valid
    return if entitlement_overrides.blank?

    Entitlements::SchemaComposer.validate_overrides(entitlement_overrides).each do |message|
      errors.add(:entitlement_overrides, message)
    end
  end

  def generate_slug
    self.slug ||= name.to_s.parameterize.presence || "ws-#{SecureRandom.hex(4)}"
  end
end
