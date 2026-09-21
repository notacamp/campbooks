# frozen_string_literal: true

module Api
  module App
    module Settings
      # Workspace-level settings: name, context (AI persona), company NIF.
      # Admin-only mutations — members may GET but not PATCH.
      class WorkspaceController < Api::App::BaseController
        # GET /api/app/settings/workspace
        def show
          render_data workspace_data
        end

        # PATCH /api/app/settings/workspace
        def update
          unless current_user.admin?
            return render_error("forbidden", "Admin access required.", status: :forbidden)
          end

          ws = current_workspace

          if params.key?(:workspace_context)
            ws.settings["workspace_context"] = params[:workspace_context]
          end

          if params.key?(:company_nif)
            nif = params[:company_nif].to_s.strip
            if nif.present?
              ws.settings["company_nif"] = nif
            else
              ws.settings.delete("company_nif")
            end
          end

          if params.key?(:name)
            ws.name = params[:name]
          end

          if ws.save
            render_data workspace_data
          else
            render_error("invalid", ws.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        private

        def workspace_data
          ws = current_workspace
          {
            id: ws.id,
            name: ws.name,
            slug: ws.slug,
            app_name: ws.app_name,
            plan: ws.plan,
            workspace_context: ws.settings["workspace_context"],
            company_nif: ws.settings["company_nif"],
            self_hosted: Rails.application.config.self_hosted,
            created_at: ws.created_at
          }
        end
      end
    end
  end
end
