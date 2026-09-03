# The calendar page itself (agenda + month + week views over synced events).
# Distinct from CalendarsController (plural), which is the per-calendar settings
# toggle nested under calendar_accounts.
class CalendarController < ApplicationController
  before_action :require_authentication

  VIEWS = %w[agenda day week month].freeze

  def index
    @view = VIEWS.include?(params[:view]) ? params[:view] : "month"
    # Native apps only offer agenda/day (the week/month grids are too dense for a
    # phone — see Calendar::ViewTabs); coerce a week/month deep link (incl. the
    # default) back to agenda there.
    @view = "agenda" if hotwire_native_app? && %w[week month].include?(@view)
    @date = parse_date(params[:date]) || Date.current

    # Shared loading (events + reminders + snoozed + scheduled + accounts + window),
    # identical to the bold Time surface — see Calendars::PageData.
    data = Calendars::PageData.for(user: Current.user, view: @view, date: @date, entitlements: current_entitlements)
    @calendar_accounts = data.calendar_accounts
    @has_calendars = data.has_calendars
    @managed_calendar_account_ids = data.managed_calendar_account_ids
    @range = data.range
    @prev_date = data.prev_date
    @next_date = data.next_date
    @events = data.events
    @reminders = data.reminders
    @snoozed_threads = data.snoozed_threads
    @scheduled_emails = data.scheduled_emails

    # Visiting the calendar clears its nav dot: stamp the pending reminders that
    # drive it (Navigation::Attention#new_calendar?).
    Reminder.accessible_to(Current.user).pending.where(viewed_at: nil).update_all(viewed_at: Time.current)
  end

  private

  def parse_date(str)
    Date.iso8601(str) if str.present?
  rescue ArgumentError
    nil
  end
end
