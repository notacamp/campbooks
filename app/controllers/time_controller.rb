# frozen_string_literal: true

# The Time surface (the fifth place): one agenda that interleaves calendar events,
# deadlines Scout found in mail (pending reminders), due-dated tasks, and Scout's
# proposed focus blocks — plus Week/Month grids as toggles. Reuses the calendar's
# loading (Calendars::PageData) and adds tasks + focus blocks; reminders are ROWS
# here, not a tab.
class TimeController < ApplicationController
  include TimeAgendaLoading # load_time_agenda / agenda_move_slots / AGENDA_DAYS

  VIEWS = %w[agenda week month].freeze

  def index
    @view = VIEWS.include?(params[:view]) ? params[:view] : "agenda"
    # Native apps get the agenda only (the grids are too dense for a phone).
    @view = "agenda" if hotwire_native_app? && %w[week month].include?(@view)
    # "Today" is the user's today (their zone), not the server's UTC date — near
    # midnight the two differ, and the header, the day note and the agenda's day
    # buckets must agree on which day that is.
    @zone = current_user.effective_time_zone
    today = Time.current.in_time_zone(@zone).to_date
    @date = parse_date(params[:date]) || today

    # The classic calendar's shared loading gives us the accounts, the window, and
    # the snoozed-thread / scheduled-email collections (rendered after the merged
    # rows on the agenda, as chips on the grids) — see Calendars::PageData.
    data = Calendars::PageData.for(user: current_user, view: @view, date: @date, entitlements: current_entitlements)
    @calendar_accounts = data.calendar_accounts
    @has_calendars = data.has_calendars
    @managed_calendar_account_ids = data.managed_calendar_account_ids
    @prev_date, @next_date = adjacent_dates(@view, @date)
    @snoozed_threads = data.snoozed_threads
    @scheduled_emails = data.scheduled_emails

    propose_focus_blocks

    if @view == "agenda"
      @agenda = Time::Agenda.for(current_user, from: agenda_from, to: agenda_to)
      @undated = Time::Agenda.undated_for(current_user)
      @move_slots = agenda_move_slots(@agenda)
      @hold_slots = agenda_hold_slots(@agenda + @undated)
      # Scout's note is about today, so it rides the default (today) agenda only.
      @day_note = Time::DayNote.for(current_user) if @date == today
    else
      @range = data.range
      @events = data.events
      @reminders = data.reminders
      @tasks = tasks_in_range(@range)
      @focus_blocks = FocusBlock.accessible_to(current_user).renderable.in_range(@range.begin, @range.end)
    end

    # Visiting Time clears the calendar nav dot, exactly like /calendar.
    Reminder.accessible_to(current_user).pending.where(viewed_at: nil).update_all(viewed_at: Time.current)
  end

  private

  def parse_date(str)
    Date.iso8601(str) if str.present?
  rescue ArgumentError
    nil
  end

  # The agenda window: the anchor date's start-of-day through 30 days later, in the
  # user's zone (so "today"/"tomorrow" bucket correctly wherever they are).
  def agenda_from
    @zone.parse(@date.iso8601).beginning_of_day
  end

  def agenda_to
    @zone.parse((@date + AGENDA_DAYS.days).iso8601).end_of_day
  end

  def adjacent_dates(view, date)
    case view
    when "month" then [ date.prev_month, date.next_month ]
    when "week"  then [ date - 7, date + 7 ]
    else [ date - AGENDA_DAYS, date + AGENDA_DAYS ]
    end
  end

  # Live, due-dated asks in the grid window (suggested ones now show too, so the
  # grids match the agenda). Empty — and unqueried — when Tasks is gated off, so the
  # agenda and grids render fine without it.
  def tasks_in_range(range)
    return Task.none unless Features.tasks?

    Task.accessible_to(current_user).live.dated.where(due_at: range.begin..range.end)
  end

  # Ask Scout to hold focus time for upcoming deadlines — at most once an hour per
  # user (the proposer is idempotent; the debounce just avoids the slot search on
  # every page view). Skipped without a calendar (nothing to schedule around).
  def propose_focus_blocks
    return unless @has_calendars

    Rails.cache.fetch("time:focus_proposed:#{current_user.id}", expires_in: 1.hour) do
      Time::FocusProposer.for(current_user)
      true
    end
  end

  # For each proposed/moved focus row, the next few free slots the Move popover
  # offers — keyed by focus-block id.
end
