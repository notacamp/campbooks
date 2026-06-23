class SetupStatus
  ITEMS = [
    {
      key: :workspace,
      severity: :critical,
      page_context: :global,
      message: "Tell Scout about your workspace",
      description: "Scout needs to know your workspace name, country, and what you do to give relevant recommendations when analyzing documents and emails.",
      cta_text: "Complete your profile",
      cta_modal: true,
      cta_path: "/setup/workspace"
    },
    {
      key: :email_account,
      severity: :critical,
      page_context: :emails,
      message: "Connect an email account to get started",
      description: "Campbooks works by scanning your inbox. Connect an email account to automatically process incoming documents, classify messages, and build contact profiles.",
      cta_text: "Connect an email account",
      cta_modal: false,
      cta_path: "/email_messages?inbox_settings=accounts"
    },
    {
      key: :ai_configuration,
      severity: :warning,
      page_context: :scout,
      message: "Connect AI to read your email",
      description: "Campbooks uses AI to triage your inbox, draft replies, classify messages, and power Scout. Add a text provider — a free DeepSeek key works, or any OpenAI-compatible service.",
      cta_text: "Set up AI",
      cta_modal: true,
      cta_path: "/setup/ai_configuration"
    },
    {
      key: :document_provider,
      severity: :info,
      page_context: :documents,
      message: "Add AI for documents & images (optional)",
      description: "To analyze PDFs, scans, and image attachments, add a vision-capable provider like OpenAI. You can skip this and add it anytime.",
      cta_text: "Add document AI",
      cta_modal: true,
      cta_path: "/setup/document_provider"
    },
    {
      key: :document_types,
      severity: :warning,
      page_context: :documents,
      message: "Define your document types first",
      description: "Tell Scout what kinds of documents you handle — invoices, contracts, bank statements, receipts. This determines what data Scout extracts and how documents are organized.",
      cta_text: "Add document types",
      cta_modal: true,
      cta_path: "/setup/document_types"
    },
    {
      key: :tags,
      severity: :info,
      page_context: :documents,
      message: "Create tags to organize emails",
      description: "Tags help you categorize emails by topic, project, or priority. Scout uses tag descriptions to automatically classify incoming messages — the clearer your tags, the better the classification.",
      cta_text: "Create email tags",
      cta_modal: true,
      cta_path: "/setup/tags"
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

  def incomplete_items
    @incomplete_items ||= ITEMS.reject { |item| send(:"#{item[:key]}_complete?") }
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
