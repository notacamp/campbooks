# frozen_string_literal: true

module Api
  module App
    module Scout
      # Returns the idle-state payload for the Scout overlay (Cmd+K).
      # Serves: AI-available flag, briefing suggestions, and the last 6 recent
      # threads. The command catalog is bundled statically by the SPA.
      #
      # Mirrors ScoutOverlayController#show (idle path) for the React SPA.
      class OverlayController < Api::App::BaseController
        # GET /api/app/scout/overlay
        def show
          recent = current_user.agent_threads.scout_visible.with_messages.recent.limit(6).to_a
          briefing = ::Scout::Briefing.for(current_user)

          render_data(
            Api::App::Scout::OverlaySerializer.new(
              ai_available: Ai::ProviderSetup.available?(current_workspace, :text),
              briefing: briefing,
              recent_threads: recent
            ).as_json
          )
        end
      end
    end
  end
end
