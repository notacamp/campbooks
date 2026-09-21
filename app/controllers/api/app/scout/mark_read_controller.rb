# frozen_string_literal: true

module Api
  module App
    module Scout
      # Marks all unviewed AI replies in the user's visible Scout threads as
      # viewed, clearing the nav dot in the docked Scout bar.
      class MarkReadController < Api::App::BaseController
        # POST /api/app/scout/mark_read
        def create
          updated = AgentMessage
            .joins(:agent_thread)
            .where(agent_threads: { user_id: current_user.id })
            .merge(AgentThread.scout_visible)
            .where(author_type: :ai, viewed_at: nil)
            .update_all(viewed_at: ::Time.current)

          render_data({ marked_read: updated })
        end
      end
    end
  end
end
