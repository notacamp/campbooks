# frozen_string_literal: true

module Api
  module App
    # Onboarding surface for the first-party app API. Backs the SPA's
    # onboarding wizard and the "Scout is reading your inbox" first-sync stage.
    #
    # All actions require a valid session bearer (no skip_before_action).
    #
    #   GET    /api/app/onboarding                     — current onboarding state
    #   PATCH  /api/app/onboarding                     — save workspace step data
    #   POST   /api/app/onboarding/snooze              — defer the wizard
    #   GET    /api/app/onboarding/first_sync_status   — first-sync polling
    #   POST   /api/app/onboarding/apply_persona       — apply a setup template
    #   POST   /api/app/onboarding/skip_first_sync     — dismiss the sync stage
    #   POST   /api/app/onboarding/suggest_document_types — AI suggestions
    #   POST   /api/app/onboarding/suggest_tags            — AI suggestions
    class OnboardingController < BaseController
      VALID_STEPS = %w[
        template welcome workspace email_accounts ai_configuration classification review
      ].freeze

      # GET /api/app/onboarding
      def show
        workspace = current_workspace
        render_data(onboarding_state(workspace))
      end

      # PATCH /api/app/onboarding
      # Accepts a `step` param to apply step-specific data (mirrors the web
      # OnboardingController#update dispatch).
      def update
        step = params[:step].to_s
        unless VALID_STEPS.include?(step)
          return render_error("invalid_step", "Unknown onboarding step.",
                              status: :unprocessable_entity)
        end

        apply_step(step, params)
        render_data(onboarding_state(current_workspace))
      rescue ActiveRecord::RecordInvalid => e
        render_error("invalid", e.record.errors.full_messages.to_sentence,
                     status: :unprocessable_entity)
      end

      # POST /api/app/onboarding/snooze
      # Defers the onboarding wizard for this session (the SPA stores this
      # preference; the API just marks the workspace settings so the web app
      # and SPA agree).
      def snooze
        ws = current_workspace
        ws.settings["onboarding_snoozed_at"] = ::Time.current.iso8601
        ws.save!
        render_data({ snoozed: true })
      end

      # GET /api/app/onboarding/first_sync_status
      # Polled while Scout's first scan runs. Returns the same shape as the
      # web Onboarding::FirstSyncStatus#as_json.
      def first_sync_status
        status = Onboarding::FirstSyncStatus.new(current_user)
        render_data(status.as_json)
      end

      # POST /api/app/onboarding/apply_persona
      # Body: { template_keys: ["freelancer", ...] }
      def apply_persona
        keys   = Array(params[:template_keys]).map(&:to_s) & Onboarding::Templates.keys
        result = keys.any? ? Onboarding::TemplateApplier.new(current_workspace, keys).apply! : { tags: [], document_types: [] }

        render_data({
          applied_keys: keys,
          tags: result[:tags].map { |t| { id: t.id, name: t.name } },
          document_types: result[:document_types].map { |dt| { id: dt.id, name: dt.name } }
        })
      rescue Onboarding::TemplateApplier::UnknownTemplate => e
        render_error("invalid_template", e.message, status: :unprocessable_entity)
      end

      # POST /api/app/onboarding/skip_first_sync
      # The user wants to skip the first-sync blocking stage. The SPA will stop
      # polling and navigate to the inbox. The workspace flag records that they
      # made an explicit choice so we don't re-trap them.
      def skip_first_sync
        ws = current_workspace
        ws.settings["first_sync_skipped_by_user"] = current_user.id.to_s
        ws.save!
        render_data({ skipped: true })
      end

      # POST /api/app/onboarding/suggest_document_types
      def suggest_document_types
        suggestions = Ai::OnboardingAssistant.new(current_workspace).suggest_document_types
        render_data({ suggestions: suggestions })
      rescue StandardError => e
        Rails.logger.error("[OnboardingController] suggest_document_types failed: #{e.message}")
        render_error("suggestion_failed", "Could not generate suggestions at this time.",
                     status: :service_unavailable)
      end

      # POST /api/app/onboarding/suggest_tags
      def suggest_tags
        suggestions = Ai::OnboardingAssistant.new(current_workspace).suggest_tags
        render_data({ suggestions: suggestions })
      rescue StandardError => e
        Rails.logger.error("[OnboardingController] suggest_tags failed: #{e.message}")
        render_error("suggestion_failed", "Could not generate suggestions at this time.",
                     status: :service_unavailable)
      end

      private

      # ── Onboarding state payload ─────────────────────────────────────────────

      def onboarding_state(workspace)
        {
          completed: workspace.settings["onboarding_completed_at"].present?,
          completed_at: workspace.settings["onboarding_completed_at"],
          snoozed: workspace.settings["onboarding_snoozed_at"].present?,
          steps: VALID_STEPS,
          workspace: {
            id: workspace.id,
            name: workspace.name,
            workspace_type: workspace.settings["workspace_type"],
            workspace_context: workspace.settings["workspace_context"],
            country: workspace.settings["country"]
          },
          email_accounts_connected: current_user.email_accounts.count,
          ai_configured: Ai::ProviderSetup.new(workspace).text_configured?,
          managed_ai_available: Ai::Platform.available?,
          templates: Onboarding::Templates.keys
        }
      end

      # ── Step applicators (mirrors OnboardingController step dispatch) ─────────

      def apply_step(step, params)
        case step
        when "workspace"         then apply_workspace_step(params)
        when "ai_configuration"  then apply_ai_step(params)
        when "classification"    then apply_classification_step(params)
        when "review"            then apply_review_step
        when "template"          then apply_template_step(params)
        end
        # email_accounts and welcome steps have no server-side data to save.
      end

      def apply_workspace_step(params)
        data = params[:workspace] || {}
        ws   = current_workspace
        ws.name = data[:name].presence || ws.name
        ws.settings["workspace_type"]    = data[:workspace_type].presence   || ws.settings["workspace_type"]
        ws.settings["workspace_context"] = data[:workspace_context].presence || ws.settings["workspace_context"]
        ws.settings["country"]           = data[:country].presence          || ws.settings["country"]
        ws.settings["city"]              = data[:city].presence             || ws.settings["city"]
        ws.settings["postal_code"]       = data[:postal_code].presence      || ws.settings["postal_code"]
        ws.settings["company_tax_id"]    = data[:company_tax_id].presence   || ws.settings["company_tax_id"]
        ws.settings["app_name"]          = ws.name
        ws.save!
      end

      def apply_ai_step(params)
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

      def apply_classification_step(params)
        ws = current_workspace

        Array(params[:selected_template_types]).each do |name|
          next if name.blank?

          preset = SetupPresets.document_type(name)
          ws.document_types.find_or_create_by!(name: name.strip.downcase) do |dt|
            dt.color  = preset&.dig(:color) || SetupPresets.color_for(name)
            dt.prompt = preset&.dig(:prompt)
          end
        end

        Array(params[:selected_template_tags]).each do |name|
          next if name.blank?

          preset = SetupPresets.tag(name)
          ws.tags.find_or_create_by!(name: name.strip.downcase) do |tag|
            tag.color  = preset&.dig(:color) || SetupPresets.color_for(name)
            tag.prompt = preset&.dig(:prompt)
            tag.source = :local
          end
        end
      end

      def apply_template_step(params)
        keys = Array(params[:template_keys]).map(&:to_s).select { |k| Onboarding::Templates.keys.include?(k) }
        Onboarding::TemplateApplier.new(current_workspace, keys).apply! if keys.any?
      end

      def apply_review_step
        current_workspace.settings["onboarding_completed_at"] = ::Time.current.iso8601
        current_workspace.save!
      end
    end
  end
end
