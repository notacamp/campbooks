# frozen_string_literal: true

module Api
  module App
    module Settings
      # Data privacy + AI processing preferences.
      class DataPrivacyController < Api::App::BaseController
        # GET /api/app/settings/data_privacy
        def show
          render_data privacy_data
        end

        # PATCH /api/app/settings/data_privacy
        def update
          if current_workspace.update(data_privacy_params)
            render_data privacy_data
          else
            render_error("invalid", current_workspace.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        private

        def privacy_data
          ws = current_workspace
          {
            ai_processing_enabled: ws.ai_processing_enabled,
            scout_thread_posts: ws.try(:scout_thread_posts),
            required_data_region: ws.try(:required_data_region),
            email_retention_months: ws.try(:email_retention_months),
            google_drive_connected: ws.google_drive_accounts.connected.exists?,
            notion_connected: ws.notion_integrations.active.exists?,
            zoho_drive_connected: ws.zoho_drive_accounts.active.exists?,
            calendar_connected: current_user.calendar_accounts.active.exists?
          }
        end

        def data_privacy_params
          params.permit(:ai_processing_enabled, :scout_thread_posts, :required_data_region, :email_retention_months)
        end
      end
    end
  end
end
