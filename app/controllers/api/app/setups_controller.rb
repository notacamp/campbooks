# frozen_string_literal: true

module Api
  module App
    # Setup-card surface for the first-party app API.
    #
    # Setup cards are the one-time "complete your profile" modals that appear in
    # the Settings banner. The web SetupController renders Turbo Streams; here we
    # return JSON so the SPA can render its own modal.
    #
    # All actions require a valid session bearer.
    #
    #   GET   /api/app/setup/:id      — current config for a setup step
    #   PATCH /api/app/setup/:id      — save step data
    #   POST  /api/app/setup/dismiss  — dismiss a setup key
    class SetupsController < BaseController
      VALID_STEPS = %w[workspace ai_configuration document_provider document_types tags].freeze

      # GET /api/app/setup/:id
      def show
        step = params[:id]
        return render_not_found unless VALID_STEPS.include?(step)

        render_data(step_state(step))
      end

      # PATCH /api/app/setup/:id
      def update
        step = params[:id]
        return render_not_found unless VALID_STEPS.include?(step)

        apply_step(step, params)
        render_data(step_state(step))
      rescue ActiveRecord::RecordInvalid => e
        render_error("invalid", e.record.errors.full_messages.to_sentence,
                     status: :unprocessable_entity)
      end

      # POST /api/app/setup/dismiss
      # Body: { key }
      def dismiss
        ws = current_workspace
        dismissed = Array(ws.settings["dismissed_setup_keys"])
        key = params[:key].to_s
        unless dismissed.include?(key)
          dismissed << key
          ws.settings["dismissed_setup_keys"] = dismissed
          ws.save!
        end
        render_data({ dismissed: true, key: key })
      end

      private

      def step_state(step)
        case step
        when "workspace"
          {
            step: step,
            workspace: {
              name: current_workspace.name,
              workspace_type: current_workspace.settings["workspace_type"],
              workspace_context: current_workspace.settings["workspace_context"],
              country: current_workspace.settings["country"],
              city: current_workspace.settings["city"],
              postal_code: current_workspace.settings["postal_code"],
              company_tax_id: current_workspace.settings["company_tax_id"]
            }
          }
        when "ai_configuration", "document_provider"
          setup = Ai::ProviderSetup.new(current_workspace)
          {
            step: step,
            managed_available: Ai::Platform.available?,
            ai_mode: current_ai_mode(setup),
            text_provider: setup.text_provider || "deepseek",
            document_provider: setup.document_provider || "openai",
            text_configured: setup.text_configured?,
            documents_configured: setup.documents_configured?
          }
        when "document_types"
          {
            step: step,
            presets: SetupPresets::DOCUMENT_TYPES.map { |p| { name: p[:name], color: p[:color] } },
            existing: current_workspace.document_types.order(:name).map { |dt| { id: dt.id, name: dt.name } }
          }
        when "tags"
          {
            step: step,
            presets: SetupPresets::TAGS.map { |p| { name: p[:name], color: p[:color] } },
            existing: current_workspace.tags.where(source: :local).order(:name).map { |t| { id: t.id, name: t.name } }
          }
        else
          { step: step }
        end
      end

      def apply_step(step, params)
        case step
        when "workspace"          then apply_workspace(params)
        when "ai_configuration"   then apply_ai(params)
        when "document_provider"  then apply_ai(params)  # same handler, different focus
        when "document_types"     then apply_document_types(params)
        when "tags"               then apply_tags(params)
        end
      end

      def apply_workspace(params)
        data = params[:workspace] || {}
        ws   = current_workspace
        ws.name = data[:name].presence || ws.name
        ws.settings["workspace_type"]    = data[:workspace_type].presence    || ws.settings["workspace_type"]
        ws.settings["workspace_context"] = data[:workspace_context].presence || ws.settings["workspace_context"]
        ws.settings["country"]           = data[:country].presence           || ws.settings["country"]
        ws.settings["city"]              = data[:city].presence              || ws.settings["city"]
        ws.settings["postal_code"]       = data[:postal_code].presence       || ws.settings["postal_code"]
        ws.settings["company_tax_id"]    = data[:company_tax_id].presence    || ws.settings["company_tax_id"]
        ws.settings["app_name"]          = ws.name
        ws.save!
      end

      def apply_ai(params)
        setup          = Ai::ProviderSetup.new(current_workspace)
        ai_mode        = params[:ai_mode]
        managed_offered = Ai::Platform.available? && !Rails.application.config.self_hosted

        if ai_mode == "managed" && managed_offered
          setup.apply_managed
          return
        end

        text = params[:text_provider]
        if text && text[:provider].present? && (text[:api_key].present? || Rails.application.config.self_hosted)
          setup.apply_text(provider: text[:provider], api_key: text[:api_key],
                           endpoint_url: text[:endpoint_url])
        end

        docs = params[:document_provider]
        if docs && docs[:provider].present? && (docs[:api_key].present? || Rails.application.config.self_hosted)
          setup.apply_documents(provider: docs[:provider], api_key: docs[:api_key],
                                endpoint_url: docs[:endpoint_url])
        end
      end

      def apply_document_types(params)
        ws    = current_workspace
        names = selected_names(params, :document_type)
        return if names.empty?

        names.each do |name|
          preset = SetupPresets.document_type(name)
          ws.document_types.find_or_create_by!(name: name) do |dt|
            dt.color  = preset&.dig(:color) || SetupPresets.color_for(name)
            dt.prompt = preset&.dig(:prompt)
          end
        end
      end

      def apply_tags(params)
        ws    = current_workspace
        names = selected_names(params, :tag)
        return if names.empty?

        names.each do |name|
          preset = SetupPresets.tag(name)
          ws.tags.find_or_create_by!(name: name) do |tag|
            tag.color  = preset&.dig(:color) || SetupPresets.color_for(name)
            tag.prompt = preset&.dig(:prompt)
            tag.source = :local
          end
        end
      end

      def selected_names(params, key)
        data = params[key] || {}
        (Array(data[:presets]) + [ data[:custom] ])
          .map { |n| n.to_s.strip.downcase }
          .reject(&:blank?)
          .uniq
      end

      def current_ai_mode(setup)
        return "managed" if setup.using_managed?
        return "byo"     if setup.text_configured?

        nil
      end
    end
  end
end
