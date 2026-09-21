# frozen_string_literal: true

module Api
  module App
    module Calendar
      # GET /api/app/calendar
      # Screen payload for the Calendar surface: accounts sidebar + all four
      # event collections (events, reminders, snoozed, scheduled) for the
      # requested view and anchor date. Pure loader — no side effects.
      class CalendarController < Api::App::BaseController
        VIEWS = %w[agenda day week month].freeze

        def index
          view = VIEWS.include?(params[:view]) ? params[:view] : "month"
          date = parse_date(params[:date]) || ::Date.current

          data = Calendars::PageData.for(
            user: current_user,
            view: view,
            date: date,
            entitlements: workspace_entitlements
          )

          payload = Api::App::Calendar::CalendarPageSerializer.new(
            data, user: current_user
          ).as_json

          # Also expose the resolved view + date so the client can sync toolbar state.
          render_data(payload.merge(view: view, date: date.iso8601))
        end

        private

        def parse_date(str)
          ::Date.iso8601(str) if str.present?
        rescue ArgumentError
          nil
        end

        def workspace_entitlements
          current_workspace&.entitlements || Entitlements::NullResolver.new
        end
      end
    end
  end
end
