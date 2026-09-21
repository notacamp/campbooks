# frozen_string_literal: true

module Api
  module App
    module Settings
      module Integrations
        # Notion integration settings: list connected workspaces, connect via
        # manual token (self-hosted fallback), disconnect.
        class NotionController < Api::App::BaseController
          # GET /api/app/settings/integrations/notion
          def show
            render_data notion_data
          end

          # PATCH /api/app/settings/integrations/notion  (manual token connect)
          def update
            integration = current_workspace.notion_integrations.new(integration_params)

            unless integration.save
              return render_error("invalid", integration.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end

            begin
              bot_info = Notion::Client.new(integration).get_bot_info
              name = bot_info["workspace_name"] || bot_info.dig("bot", "workspace_name")
              id   = bot_info["workspace_id"] || bot_info.dig("bot", "workspace_id")
              integration.update_columns(notion_workspace_name: name, notion_workspace_id: id) if name || id
              render_data notion_data
            rescue => e
              integration.destroy
              render_error("invalid", "Could not connect to Notion: #{e.message}", status: :unprocessable_entity)
            end
          end

          # DELETE /api/app/settings/integrations/notion/workspaces/:id
          def destroy
            integration = current_workspace.notion_integrations.find(params[:id])
            integration.deactivate!
            render json: {}, status: :no_content
          end

          private

          def notion_data
            ws = current_workspace
            {
              oauth_configured: Notion::OauthClient.configured?,
              integrations: ws.notion_integrations.active.order(:created_at).map do |i|
                {
                  id: i.id,
                  notion_workspace_name: i.notion_workspace_name,
                  notion_workspace_id: i.notion_workspace_id,
                  created_at: i.created_at
                }
              end
            }
          end

          def integration_params
            params.require(:notion_integration).permit(:access_token)
          end
        end
      end
    end
  end
end
