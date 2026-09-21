# frozen_string_literal: true

module Api
  module App
    module Scout
      # Reports the count of unviewed AI replies across all of the user's visible
      # Scout threads. The SPA uses this to light the unread-reply dot in the
      # docked Scout bar.
      class UnreadController < Api::App::BaseController
        # GET /api/app/scout/unread
        def show
          count = AgentMessage
            .joins(:agent_thread)
            .where(agent_threads: { user_id: current_user.id })
            .merge(AgentThread.scout_visible)
            .where(author_type: :ai, viewed_at: nil)
            .count

          render_data({ count: count })
        end
      end
    end
  end
end
