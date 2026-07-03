class SetupStatus
  # Setup tasks in priority order: the inbox is the gateway to everything, AI
  # lets Scout read it, taxonomy tunes the sorting, the workspace intro adds
  # context, document AI is a bonus. Metadata only — the user-facing strings
  # resolve through #localized at read time so they follow the request locale
  # (config/locales/<locale>/onboarding.yml, under setup_status.items).
  ITEMS = [
    {
      key: :email_account,
      severity: :critical,
      page_context: :emails,
      cta_modal: false,
      cta_path: "/email_messages?inbox_settings=accounts"
    },
    {
      key: :ai_configuration,
      severity: :warning,
      page_context: :scout,
      cta_modal: true,
      cta_path: "/setup/ai_configuration"
    },
    {
      key: :document_types,
      severity: :warning,
      page_context: :documents,
      cta_modal: true,
      cta_path: "/setup/document_types"
    },
    {
      key: :tags,
      severity: :info,
      page_context: :documents,
      cta_modal: true,
      cta_path: "/setup/tags"
    },
    {
      key: :workspace,
      severity: :warning,
      page_context: :global,
      cta_modal: true,
      cta_path: "/setup/workspace"
    },
    {
      key: :document_provider,
      severity: :info,
      page_context: :documents,
      cta_modal: true,
      cta_path: "/setup/document_provider"
    }
  ].freeze

  def initialize(workspace)
    @workspace = workspace
  end

  def complete?
    incomplete_items.empty?
  end

  def most_critical_item
    incomplete_items.first
  end

  def items_for_page(page_key)
    incomplete_items.select { |item| item[:page_context] == page_key.to_sym }
  end

  # Every task with its strings resolved — what the setup hub renders.
  def all_items
    ITEMS.map { |item| localized(item) }
  end

  def incomplete_items
    @incomplete_items ||= ITEMS.reject { |item| send(:"#{item[:key]}_complete?") }
                              .map { |item| localized(item) }
  end

  def first_incomplete_step
    item = incomplete_items.first
    return nil unless item
    case item[:key]
    when :workspace then "workspace"
    when :email_account then "email_accounts"
    when :ai_configuration, :document_provider then "ai_configuration"
    when :document_types, :tags then "classification"
    else "workspace"
    end
  end

  def severity_for(key)
    item = ITEMS.find { |i| i[:key] == key }
    item&.fetch(:severity, :info)
  end

  def first_incomplete_severity
    item = incomplete_items.first
    item&.fetch(:severity, :info)
  end

  private

  def localized(item)
    item.merge(
      message: I18n.t("setup_status.items.#{item[:key]}.message"),
      description: I18n.t("setup_status.items.#{item[:key]}.description"),
      cta_text: I18n.t("setup_status.items.#{item[:key]}.cta")
    )
  end

  def workspace_complete?
    @workspace.name != "My Organization" &&
      @workspace.settings&.dig("country").present? &&
      @workspace.settings&.dig("workspace_context").present?
  end

  def email_account_complete?
    @workspace.email_accounts.active.any?
  end

  # "AI is set up" for onboarding/setup nudges uses the STRICT gate: a shared
  # platform env key (ANTHROPIC_API_KEY) makes interactive AI work, but it
  # shouldn't mark the workspace's AI as configured — otherwise the user is never
  # nudged to set it up, yet background processing (also strict) silently does
  # nothing. Keeps the nudge and the processing gate in agreement.
  def ai_configuration_complete?
    Ai::ProviderSetup.new(@workspace).configured?(:text)
  end

  def document_provider_complete?
    Ai::ProviderSetup.new(@workspace).configured?(:documents)
  end

  def document_types_complete?
    @workspace.document_types.any?
  end

  def tags_complete?
    @workspace.tags.any?
  end
end
