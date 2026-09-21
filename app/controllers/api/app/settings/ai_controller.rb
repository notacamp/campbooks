# frozen_string_literal: true

module Api
  module App
    module Settings
      # AI overview: mode, adapters, configurations, embeddings.
      class AiController < Api::App::BaseController
        before_action :set_org

        # GET /api/app/settings/ai
        def show
          adapters = @org.ai_adapters.order(:name)
          configs = @org.ai_configurations.includes(:ai_adapter).order(:purpose).index_by(&:purpose)

          render_data({
            mode: Ai::ProviderSetup.new(@org).using_managed? ? "managed" : "byo",
            managed_available: Ai::Platform.available?,
            embedding_model: @org.embedding_model,
            adapters: adapters.map { |a| adapter_data(a) },
            configurations: configs.transform_values { |c| config_data(c) }
          })
        end

        # POST /api/app/settings/ai/switch_mode
        def switch_mode
          setup = Ai::ProviderSetup.new(@org)

          case params[:mode]
          when "managed"
            if Ai::Platform.available? && !Rails.application.config.self_hosted
              disable_byo_role_adapters
              setup.apply_managed
              render_data({ mode: "managed" })
            else
              render_error("managed_unavailable", "Managed AI is not available.", status: :unprocessable_entity)
            end
          when "byo"
            @org.ai_adapters.where(managed: true).update_all(enabled: false)
            render_data({ mode: "byo" })
          else
            render_error("invalid", "Invalid mode.", status: :unprocessable_entity)
          end
        end

        # PATCH /api/app/settings/ai/embeddings
        def embeddings
          key   = params[:embedding_model].to_s
          entry = Ai::EmbeddingModels.find(key)

          unless entry
            return render_error("invalid", "Unknown embedding model.", status: :unprocessable_entity)
          end

          unless @org.region_allows?(entry.provider)
            return render_error("region_blocked", "This provider is not available in your region.", status: :unprocessable_entity)
          end

          unless EmbeddingService.available_for?(@org, entry: entry)
            return render_error("provider_unavailable", "Provider is not configured.", status: :unprocessable_entity)
          end

          @org.update!(embedding_model: entry.key)
          ::Search::WorkspaceReembedJob.perform_later(@org)
          render_data({ embedding_model: @org.embedding_model, reembed_queued: true })
        end

        private

        def set_org
          @org = current_workspace
        end

        def disable_byo_role_adapters
          ids = @org.ai_configurations
                    .where(purpose: AiConfiguration::TEXT_PURPOSES + AiConfiguration::DOCUMENT_PURPOSES)
                    .joins(:ai_adapter).where(ai_adapters: { managed: false })
                    .pluck(:ai_adapter_id)
          @org.ai_adapters.where(id: ids).update_all(enabled: false)
        end

        def adapter_data(adapter)
          {
            id: adapter.id,
            name: adapter.name,
            provider: adapter.provider,
            enabled: adapter.enabled,
            managed: adapter.managed,
            in_use: adapter.in_use?,
            endpoint_url: adapter.endpoint_url,
            has_api_key: adapter.api_key.present?
          }
        end

        def config_data(config)
          {
            id: config.id,
            purpose: config.purpose,
            ai_adapter_id: config.ai_adapter_id,
            model: config.try(:model)
          }
        end
      end
    end
  end
end
