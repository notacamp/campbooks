# Per-user show/hide of a single calendar on the /calendar page (the sidebar
# checkboxes). Purely personal display state on the user row — the calendar
# keeps syncing for everyone; contrast with CalendarsController#update, the
# account-wide, manager-gated syncing/color toggle.
class CalendarVisibilitiesController < ApplicationController
  before_action :require_authentication

  def update
    calendar = readable_calendars.find(params[:id])
    # Explicit hidden=0|1 (not a blind toggle) so a double submit stays idempotent.
    Current.user.set_calendar_hidden!(calendar, params[:hidden] == "1")
    redirect_to calendar_path(view: params[:view].presence, date: params[:date].presence)
  end

  private

  # Calendars on accounts shared with this user — anything else 404s (existence
  # of other people's calendars must not leak; the app-wide 404-not-403 rule).
  def readable_calendars
    Calendar.where(calendar_account: Current.user.readable_calendar_accounts)
  end
end
