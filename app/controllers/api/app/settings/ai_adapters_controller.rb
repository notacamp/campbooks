# frozen_string_literal: true

module Api
  module App
    module Settings
      # AI adapter CRUD (BYO providers). Managed adapters are read-only.
      class AiAdaptersController < Api::App::BaseController
        before_action :set_adapter, only: %i[update destroy]

        # GET /api/app/settings/ai_adapters
        def index
          adapters = current_workspace.ai_adapters.order(:name)
          render_data adapters.map { |a| adapter_data(a) }
        end

        # POST /api/app/settings/ai_adapters
        def create
          adapter = current_workspace.ai_adapters.new(adapter_params)
          if adapter.save
            render_data adapter_data(adapter), status: :created
          else
            render_error("invalid", adapter.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        # PATCH /api/app/settings/ai_adapters/:id
        def update
          if @adapter.update(adapter_params)
            render_data adapter_data(@adapter)
          else
            render_error("invalid", @adapter.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        # DELETE /api/app/settings/ai_adapters/:id
        def destroy
          if @adapter.ai_configurations.any?
            return render_error("in_use", "This adapter is in use and cannot be deleted.", status: :unprocessable_entity)
          end

          @adapter.destroy!
          render json: {}, status: :no_content
        end

        private

        def set_adapter
          @adapter = current_workspace.ai_adapters.find(params[:id])
          if @adapter.managed?
            render_error("invalid", "Managed adapters cannot be modified here.", status: :unprocessable_entity)
          end
        end

        def adapter_params
          params.require(:ai_adapter).permit(:name, :provider, :api_key, :endpoint_url, :enabled)
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
      end
    end
  end
end
