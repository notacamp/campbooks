# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Per-user show/hide of a single calendar.
      # PATCH /api/app/calendar_visibilities/:id
      # Idempotent: explicit hidden=true|false (not a blind toggle).
      class CalendarVisibilitiesController < Api::App::BaseController
        def update
          calendar = readable_calendars.find(params[:id])

          # Accept hidden as a boolean param or the string "1"/"true".
          hidden = ActiveModel::Type::Boolean.new.cast(params[:hidden])
          current_user.set_calendar_hidden!(calendar, hidden)

          render_data({ id: calendar.id, hidden: hidden })
        end

        private

        def readable_calendars
          ::Calendar.where(calendar_account: current_user.readable_calendar_accounts)
        end
      end
    end
  end
end
