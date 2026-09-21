# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Per-calendar sync toggle and color override, nested under calendar_accounts.
      # PATCH /api/app/calendar_accounts/:calendar_account_id/calendars/:id
      # Requires manager access on the account.
      class CalendarsController < Api::App::BaseController
        def update
          account = current_user.calendar_accounts.find(params[:calendar_account_id])

          unless account.managed_by?(current_user)
            return render_error("not_permitted",
                                "You need manager access to update calendar settings.",
                                status: :forbidden)
          end

          calendar = account.calendars.find(params[:id])
          calendar.update!(calendar_params)

          render_data(calendar_as_json(calendar))
        end

        private

        def calendar_params
          params.permit(:syncing, :color)
        end

        def calendar_as_json(cal)
          {
            id: cal.id,
            name: cal.name,
            color: cal.display_color,
            syncing: cal.syncing,
            is_primary: cal.is_primary,
            is_writable: cal.is_writable,
            calendar_account_id: cal.calendar_account_id
          }
        end
      end
    end
  end
end
