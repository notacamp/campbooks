class SetupController < ApplicationController
  VALID_STEPS = %w[workspace ai_configuration document_provider document_types tags].freeze
  before_action :ensure_turbo_frame, only: :show

  def show
    @step = params[:id]
    return head :not_found unless VALID_STEPS.include?(@step)

    case @step
    when "workspace"
      @form_step = params[:form_step].to_i
      render partial: "shared/modals/workspace_form"
    when "ai_configuration"
      setup = Ai::ProviderSetup.new(Current.workspace)
      render partial: "shared/modals/ai_configuration_form", locals: {
        managed_available: Ai::Platform.available?,
        ai_mode: current_ai_mode(setup),
        text_provider: setup.text_provider || "deepseek",
        document_provider: setup.document_provider || "openai"
      }
    when "document_provider"
      # The "Add AI for documents & images" item: the vision provider on its own,
      # so the text provider can't be reconfigured from here. Posts to the same
      # update_ai_configuration handler (it only applies the params it receives).
      setup = Ai::ProviderSetup.new(Current.workspace)
      render partial: "shared/modals/ai_configuration_form", locals: {
        focus: :documents,
        document_provider: setup.document_provider || "openai"
      }
    when "document_types"
      render partial: "shared/modals/document_types_form"
    when "tags"
      render partial: "shared/modals/tags_form"
    end
  end

  def update
    @step = params[:id]
    return head :not_found unless VALID_STEPS.include?(@step)

    case @step
    when "workspace"
      update_workspace
    when "ai_configuration"
      update_ai_configuration
    when "document_types"
      update_document_types
    when "tags"
      update_tags
    end
  end

  def dismiss
    org = Current.workspace
    if org
      dismissed = Array(org.settings["dismissed_setup_keys"])
      dismissed << params[:key].to_s unless dismissed.include?(params[:key].to_s)
      org.settings["dismissed_setup_keys"] = dismissed
      org.save!
    end
    redirect_back fallback_location: root_path
  end

  private

  def ensure_turbo_frame
    redirect_to root_path unless turbo_frame_request?
  end

  # On a successful final step: refresh the setup banner, toast, and close the
  # dialog. Never redirect — a frame-targeted redirect renders "Content missing".
  # @_setup_status is memoized by a before_action that ran before this save, so
  # reset it here for the banner to reflect the just-saved state. See ADR 0001.
  def setup_complete_streams(message)
    @_setup_status = nil
    [
      turbo_stream.replace("setup_banner", partial: "shared/setup_banner"),
      notify_stream(message),
      turbo_stream.update("setup_modal_frame", partial: "shared/modals/close")
    ]
  end

  # Re-render a sub-wizard inside the frame with a visible inline error.
  def render_setup_error(form, message, locals = {})
    render partial: "shared/modals/#{form}", status: :unprocessable_entity,
           locals: { error: message }.merge(locals)
  end

  def update_workspace
    org = Current.workspace
    data = params[:workspace]
    @form_step = params[:form_step].to_i
    return render_setup_error("workspace_form", t(".workspace_error")) unless data

    org.name = data[:name].presence || org.name
    org.settings["workspace_type"] = data[:workspace_type].presence || org.settings["workspace_type"]
    org.settings["workspace_context"] = data[:workspace_context].presence || org.settings["workspace_context"]
    org.settings["country"] = data[:country].presence || org.settings["country"]
    org.settings["city"] = data[:city].presence || org.settings["city"]
    org.settings["postal_code"] = data[:postal_code].presence || org.settings["postal_code"]
    org.settings["company_tax_id"] = data[:company_tax_id].presence || org.settings["company_tax_id"]
    org.settings["app_name"] = org.name
    org.save!

    next_step = @form_step + 1
    if next_step >= 3
      render turbo_stream: setup_complete_streams(t(".workspace_saved"))
    else
      @step = "workspace"
      @form_step = next_step
      render partial: "shared/modals/workspace_form"
    end
  end

  def update_ai_configuration
    setup = Ai::ProviderSetup.new(Current.workspace)
    ai_mode = params[:ai_mode]
    managed_offered = Ai::Platform.available? && !self_hosted?

    # When the managed/BYO choice is shown, a click on Save must pick one (otherwise
    # the hidden BYO fields would submit a silent no-op).
    if managed_offered && ai_mode.blank?
      return render_setup_error("ai_configuration_form",
        t("shared.modals.ai_configuration_form.choose_mode"),
        { managed_available: true, ai_mode: nil,
          text_provider: setup.text_provider || "deepseek",
          document_provider: setup.document_provider || "openai" })
    end

    if ai_mode == "managed" && managed_offered
      setup.apply_managed
      return render turbo_stream: setup_complete_streams(t(".ai_saved"))
    end

    text = params[:text_provider]
    if text && text[:provider].present? && (text[:api_key].present? || self_hosted?)
      setup.apply_text(provider: text[:provider], api_key: text[:api_key], endpoint_url: text[:endpoint_url])
    end

    docs = params[:document_provider]
    if docs && docs[:provider].present? && (docs[:api_key].present? || self_hosted?)
      setup.apply_documents(provider: docs[:provider], api_key: docs[:api_key], endpoint_url: docs[:endpoint_url])
    end

    render turbo_stream: setup_complete_streams(t(".ai_saved"))
  end

  # Current AI mode for prefilling the setup modal: managed, byo, or nil (unset).
  def current_ai_mode(setup)
    return "managed" if setup.using_managed?
    return "byo" if setup.text_configured?

    nil
  end

  def update_document_types
    names = selected_names(:document_type)
    return render_setup_error("document_types_form", t(".pick_at_least_one_type")) if names.empty?

    names.each do |name|
      preset = SetupPresets.document_type(name)
      Current.workspace.document_types.find_or_create_by!(name: name) do |dt|
        dt.color  = preset&.dig(:color) || SetupPresets.color_for(name)
        dt.prompt = preset&.dig(:prompt)
      end
    end
    render turbo_stream: setup_complete_streams(t(".document_types_added", count: names.size))
  end

  def update_tags
    names = selected_names(:tag)
    return render_setup_error("tags_form", t(".pick_at_least_one_tag")) if names.empty?

    names.each do |name|
      preset = SetupPresets.tag(name)
      Current.workspace.tags.find_or_create_by!(name: name) do |tag|
        tag.color  = preset&.dig(:color) || SetupPresets.color_for(name)
        tag.prompt = preset&.dig(:prompt)
        tag.source = :local
      end
    end
    render turbo_stream: setup_complete_streams(t(".tags_added", count: names.size))
  end

  # Collected from the starter-pack checkboxes (presets[]) plus the custom field,
  # normalized to unique lowercase names.
  def selected_names(key)
    data = params[key] || {}
    (Array(data[:presets]) + [ data[:custom] ])
      .map { |n| n.to_s.strip.downcase }
      .reject(&:blank?)
      .uniq
  end
end
