class OnboardingController < ApplicationController
  layout "onboarding"

  STEPS = %w[workspace email_accounts ai_configuration classification review].freeze

  before_action :load_templates
  before_action :set_step
  before_action :ensure_valid_step!
  before_action :set_previous_step
  before_action :clear_onboarding_snooze, only: [ :update ]

  def show
    send(:"prepare_#{@step}")
    render "onboarding/steps/#{@step}"
  end

  def update
    send(:"update_#{@step}")
    return if performed? # a step (e.g. AI mode validation) may re-render inline

    if sub_steps?(@step)
      @sub_step = (params[:sub_step].to_i || 0) + 1
      if @sub_step < sub_step_count(@step)
        send(:"prepare_#{@step}")
        render "onboarding/steps/#{@step}"
        return
      end
    end

    next_step = STEPS[STEPS.index(@step) + 1]
    if next_step
      redirect_to onboarding_path(step: next_step)
    else
      session[:onboarding_completed] = true
      redirect_to root_path, success: t(".setup_complete")
    end
  end

  def snooze
    session[:onboarding_snoozed] = true
    redirect_to root_path
  end

  # ── AI suggestion endpoints ──────────────────────────────

  def suggest_document_types
    suggestions = Ai::OnboardingAssistant.new(Current.workspace).suggest_document_types
    respond_to do |format|
      format.turbo_stream {
        render turbo_stream: turbo_stream.replace(
          "ai_suggestions_document_types",
          partial: "onboarding/suggestions/document_types",
          locals: { suggestions: suggestions }
        )
      }
    end
  rescue => e
    Rails.logger.error("[Onboarding] AI suggestion error: #{e.message}")
    respond_to do |format|
      format.turbo_stream {
        render turbo_stream: turbo_stream.replace(
          "ai_suggestions_document_types",
          partial: "onboarding/suggestions/error",
          locals: { error: t(".suggestion_error") }
        )
      }
    end
  end

  def suggest_tags
    suggestions = Ai::OnboardingAssistant.new(Current.workspace).suggest_tags
    respond_to do |format|
      format.turbo_stream {
        render turbo_stream: turbo_stream.replace(
          "ai_suggestions_tags",
          partial: "onboarding/suggestions/tags",
          locals: { suggestions: suggestions }
        )
      }
    end
  rescue => e
    Rails.logger.error("[Onboarding] AI suggestion error: #{e.message}")
    respond_to do |format|
      format.turbo_stream {
        render turbo_stream: turbo_stream.replace(
          "ai_suggestions_tags",
          partial: "onboarding/suggestions/error",
          locals: { error: t(".suggestion_error") }
        )
      }
    end
  end

  private

  # ── Navigation ───────────────────────────────────────────

  # When the user actively continues the wizard, clear any prior "skip for now"
  # so the setup gate nags again next time. (The key is :onboarding_snoozed —
  # this used to delete the wrong key, so a snooze was never cleared.)
  def clear_onboarding_snooze
    session.delete(:onboarding_snoozed)
  end

  def set_step
    @step = params[:step] || STEPS.first
  end

  def ensure_valid_step!
    unless STEPS.include?(@step)
      redirect_to onboarding_path(step: STEPS.first)
    end
  end

  def set_previous_step
    idx = STEPS.index(@step)
    @previous_step = idx > 0 ? STEPS[idx - 1] : nil
  end

  def sub_steps?(step)
    step == "workspace"
  end

  def sub_step_count(step)
    case step
    when "workspace" then 5
    else 1
    end
  end

  def load_templates
    @templates = Template.order(:name)
    @default_template = Template.find_by(name: "Business Default")
  end

  def org
    unless Current.workspace
      new_org = Workspace.create!(name: "#{Current.user.name}'s Workspace")
      Current.user.update_column(:workspace_id, new_org.id)
      Current.user.reload
      Current.workspace = new_org
      # New workspaces default to managed "Campbooks AI" (cloud + best-effort).
      Ai::ProviderSetup.apply_managed_default(new_org)
    end
    Current.workspace
  end

  # ── Step 1: Workspace ────────────────────────────────────

  def prepare_workspace
    org # ensure org exists
    @sub_step ||= params[:sub_step].to_i if params[:sub_step]
  end

  def update_workspace
    data = params[:workspace]
    return unless data

    org.name = data[:name].presence || org.name
    org.settings["workspace_type"] = data[:workspace_type].presence || org.settings["workspace_type"]
    org.settings["workspace_context"] = data[:workspace_context].presence || org.settings["workspace_context"]
    org.settings["country"] = data[:country].presence || org.settings["country"]
    org.settings["address_line1"] = data[:address_line1].presence || org.settings["address_line1"]
    org.settings["address_line2"] = data[:address_line2].presence || org.settings["address_line2"]
    org.settings["city"] = data[:city].presence || org.settings["city"]
    org.settings["postal_code"] = data[:postal_code].presence || org.settings["postal_code"]
    org.settings["company_tax_id"] = data[:company_tax_id].presence || org.settings["company_tax_id"]
    org.settings["app_name"] = org.name
    org.save!
  end

  # ── Step 2: Email Accounts ───────────────────────────────

  def prepare_email_accounts
    @accounts = Current.user.email_accounts.includes(:email_account_users)
    session[:onboarding_return_to] = onboarding_path(step: :email_accounts)
  end

  def update_email_accounts
    session.delete(:onboarding_return_to)
  end

  # ── Step 3: AI Configuration ─────────────────────────────

  def prepare_ai_configuration
    setup = Ai::ProviderSetup.new(org)
    @managed_available = Ai::Platform.available?
    # Preselect the current mode for a returning user; nil = nothing chosen yet
    # (the desired first-run UX — the user must pick).
    @ai_mode = ("managed" if setup.using_managed?) || ("byo" if setup.text_configured?)
    @text_provider = setup.text_provider || "deepseek"
    @document_provider = setup.document_provider || "openai"
    @documents_configured = setup.documents_configured?
  end

  def update_ai_configuration
    setup = Ai::ProviderSetup.new(org)
    ai_mode = params[:ai_mode]
    managed_offered = Ai::Platform.available? && !self_hosted?

    # When the choice is shown, the user must make one.
    if managed_offered && ai_mode.blank?
      @mode_error = t("onboarding.steps.ai_configuration.choose_mode")
      prepare_ai_configuration
      render "onboarding/steps/ai_configuration", status: :unprocessable_entity
      return
    end

    if ai_mode == "managed" && managed_offered
      setup.apply_managed
      return
    end

    text = params[:text_provider]
    if text && text[:provider].present? && (text[:api_key].present? || self_hosted?)
      setup.apply_text(provider: text[:provider], api_key: text[:api_key], endpoint_url: text[:endpoint_url])
    end

    docs = params[:document_provider]
    if docs && docs[:provider].present? && (docs[:api_key].present? || self_hosted?)
      setup.apply_documents(provider: docs[:provider], api_key: docs[:api_key], endpoint_url: docs[:endpoint_url])
    end
  end

  # ── Step 4: Classification (Document Types + Tags) ────────

  def prepare_classification
    @existing_types = org.document_types.order(:name)
    @existing_tags = org.tags.where(source: :local).order(:name)
    @template_document_types = @default_template&.document_types || []
    @template_tags = @default_template&.tags || []
  end

  def update_classification
    # Apply document types
    selected_type_names = Array(params[:selected_template_types])
    if params[:template_id].present? && selected_type_names.any?
      template = Template.find(params[:template_id])
      template.apply_document_types(org, selected_type_names)
    end

    customs = params[:custom_document_types]&.to_unsafe_h || {}
    customs.each_value do |attrs|
      next if attrs[:name].blank?
      org.document_types.find_or_create_by!(name: attrs[:name].strip.downcase) do |dt|
        dt.color = attrs[:color].presence || generate_color(attrs[:name])
        dt.prompt = attrs[:prompt].presence || "Custom document type: #{attrs[:name]}"
        dt.extraction_schema = parse_schema(attrs[:extraction_schema])
      end
    end

    # Apply tags
    selected_tag_names = Array(params[:selected_template_tags])
    if params[:template_id].present? && selected_tag_names.any?
      template = Template.find_by(id: params[:template_id]) || @default_template
      template&.apply_tags(org, selected_tag_names)
    end

    tag_customs = params[:custom_tags]&.to_unsafe_h || {}
    tag_customs.each_value do |attrs|
      next if attrs[:name].blank?
      org.tags.find_or_create_by!(name: attrs[:name].strip.downcase) do |tag|
        tag.color = attrs[:color].presence || generate_color(attrs[:name])
        tag.prompt = attrs[:prompt].presence || "Tag: #{attrs[:name]}"
        tag.source = :local
      end
    end
  end

  # ── Step 5: Review ───────────────────────────────────────

  def prepare_review
    @org = org
    @accounts = Current.user.email_accounts
    @configs = org.ai_configurations.includes(:ai_adapter).order(:purpose)
    @document_types = org.document_types.order(:name)
    @tags = org.tags.where(source: :local).order(:name)
  end

  def update_review
    org.settings["onboarding_completed_at"] = Time.current.iso8601
    org.save!
    session[:onboarding_completed] = true
  end

  # ── Helpers ──────────────────────────────────────────────

  def generate_color(name)
    hash = name.to_s.bytes.sum
    hue = hash % 360
    "##{hsl_to_hex(hue, 0.65, 0.55)}"
  end

  def hsl_to_hex(h, s, l)
    h = h / 360.0
    c = (1 - (2 * l - 1).abs) * s
    x = c * (1 - ((h * 6) % 2 - 1).abs)
    m = l - c / 2.0
    r, g, b = case (h * 6).floor % 6
    when 0 then [ c, x, 0 ]
    when 1 then [ x, c, 0 ]
    when 2 then [ 0, c, x ]
    when 3 then [ 0, x, c ]
    when 4 then [ x, 0, c ]
    when 5 then [ c, 0, x ]
    end
    [ r, g, b ].map { |v| ((v + m) * 255).round.to_s(16).rjust(2, "0") }.join
  end

  def parse_schema(text)
    return nil if text.blank?
    JSON.parse(text)
  rescue JSON::ParserError
    nil
  end
end
