# The calendar page itself (agenda + month + week views over synced events).
# Distinct from CalendarsController (plural), which is the per-calendar settings
# toggle nested under calendar_accounts.
class CalendarController < ApplicationController
  before_action :require_authentication

  VIEWS = %w[agenda day week month].freeze
  AGENDA_LIMIT = 100 # how many upcoming events the agenda lists from the anchor date

  def index
    @view = VIEWS.include?(params[:view]) ? params[:view] : "agenda"
    @date = parse_date(params[:date]) || Date.current
    @has_calendars = Current.user.readable_calendar_accounts.active.exists?
    @range = range_for(@view, @date)
    @prev_date, @next_date = adjacent_dates(@view, @date)

    scope = CalendarEvent.accessible_to(Current.user).visible
                         .order(:start_at)
                         .includes(calendar: :calendar_account)

    # Agenda lists your next events from the anchor date forward (no hard window),
    # so it never reads "nothing coming up" when your next event is just past the
    # month edge. Week/Month query their exact grid range.
    @events = if @view == "agenda"
      # Upcoming = from the anchor forward, minus timed events that already ended,
      # so the list never shows things that are over. All-day events stay.
      scope.where(start_at: @date.beginning_of_day..)
           .where("COALESCE(calendar_events.end_at, calendar_events.start_at) >= :now OR calendar_events.all_day = :all_day",
                  now: Time.current, all_day: true)
           .limit(AGENDA_LIMIT)
    else
      scope.in_range(@range.begin, @range.end)
    end

    # Pending reminders ride alongside events as distinct "suggestion" chips. Only
    # unconfirmed ones (confirmed reminders already exist as real CalendarEvents).
    @reminders = @has_calendars ? reminders_for_view : []

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

  # Pending, not-yet-confirmed reminders in the current view's window, to overlay on
  # the grid. Agenda lists forward from the anchor (like @events); grids use @range.
  def reminders_for_view
    # Never surface past-due reminders — they're no longer actionable. Floor at the
    # start of today so all-day reminders due today still show.
    scope = Reminder.accessible_to(Current.user).pending.where(calendar_event_id: nil)
                    .where(due_at: Time.current.beginning_of_day..).order(:due_at)
    if @view == "agenda"
      scope.limit(AGENDA_LIMIT)
    else
      scope.where(due_at: @range.begin..@range.end)
    end
  end

  # Time bounds for the events query, widened to whole days (and, for month, to
  # the full calendar grid including leading/trailing days).
  def range_for(view, date)
    case view
    when "month"
      (date.beginning_of_month.beginning_of_week.beginning_of_day)..(date.end_of_month.end_of_week.end_of_day)
    when "week"
      date.beginning_of_week.beginning_of_day..date.end_of_week.end_of_day
    when "day"
      date.beginning_of_day..date.end_of_day
    else # agenda — the next 30 days from the anchor date
      date.beginning_of_day..(date + 30.days).end_of_day
    end
  end

  def adjacent_dates(view, date)
    case view
    when "month" then [ date.prev_month, date.next_month ]
    when "week"  then [ date - 7, date + 7 ]
    when "day"   then [ date - 1, date + 1 ]
    else [ date - 30, date + 30 ]
    end
  end
end
