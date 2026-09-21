# frozen_string_literal: true

module Api
  module App
    module Settings
      module Integrations
        # Integration status overview: which providers are connected + counts.
        class IndexController < Api::App::BaseController
          # GET /api/app/settings/integrations
          def show
            ws = current_workspace
            render_data({
              google_drive: {
                connected: ws.google_drive_accounts.connected.exists?,
                account_count: ws.google_drive_accounts.connected.count
              },
              notion: {
                connected: ws.notion_integrations.active.exists?,
                workspace_count: ws.notion_integrations.active.count
              },
              zoho_drive: {
                connected: ws.zoho_drive_accounts.active.exists?,
                account_count: ws.zoho_drive_accounts.active.count
              },
              calendars: {
                connected: current_user.calendar_accounts.active.exists?,
                account_count: current_user.calendar_accounts.active.count
              },
              connections: {
                count: ws.connections.count
              }
            })
          end
        end
      end
    end
  end
end
