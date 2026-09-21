# frozen_string_literal: true

module Api
  module App
    module People
      # GET /api/app/people
      # The People directory: paginated PeopleStanding rows for the current user.
      # Need-you rows come first (ranked by score), then Latest (all, by last_activity_at).
      # Accepts ?q= (search), ?tab=latest|needing, ?page=.
      class DirectoryController < Api::App::BaseController
        PAGE_SIZE = 30

        def index
          ensure_standings_fresh
          rows = PeopleStanding.for_user(current_user)
          rows = rows.search(params[:q].to_s.strip) if params[:q].present?

          case params[:tab].to_s
          when "needing"
            pagy, page_rows = pagy(rows.needing.ranked, limit: PAGE_SIZE)
          else
            # Default: needing first, then latest as the main list.
            pagy, page_rows = pagy(rows.latest, limit: PAGE_SIZE)
          end

          serialized = page_rows.map { |r| Api::App::PeopleStandingSerializer.new(r).as_json }
          render_page(serialized, pagy)
        end

        private

        def ensure_standings_fresh
          if ::People::Standings.missing?(current_user)
            return unless Contact.where(workspace_id: current_workspace.id).where("email_count > 0").exists?

            ::People::Standings.refresh!(current_user)
          elsif ::People::Standings.stale?(current_user)
            ::People::StandingsRefreshJob.enqueue_for(current_user.id)
          end
        end
      end
    end
  end
end
