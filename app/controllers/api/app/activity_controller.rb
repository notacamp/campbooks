# frozen_string_literal: true

module Api
  module App
    # GET /api/app/activity
    # Workspace activity feed: paginated domain Events, newest first.
    # Mirrors ActivityController (web) but as a JSON API. Read-only.
    class ActivityController < Api::App::BaseController
      PAGE_SIZE = 30

      def index
        scope = current_workspace.events.accessible_to(current_user).recent
        scope = scope.where(name: params[:name]) if params[:name].present?

        pagy, events = pagy(scope, limit: PAGE_SIZE)

        render_page(
          events.map { |e| event_as_json(e) },
          pagy
        )
      end

      private

      def event_as_json(event)
        {
          id:          event.id,
          name:        event.name,
          group:       event.group,
          occurred_at: event.occurred_at&.iso8601,
          payload:     event.payload,
          actor_id:    event.actor_id
        }
      rescue StandardError
        { id: event.id }
      end
    end
  end
end
