# frozen_string_literal: true

module Calendars
  # Shared calendar-page loading for the classic Calendar surface (CalendarController)
  # and the bold Time surface (TimeController). Given a resolved view + anchor date,
  # it returns the sidebar accounts, the window (range + prev/next dates), and the
  # four day-grouped collections every grid/agenda component takes: events (with
  # recurring series folded into occurrences), pending reminders, snoozed threads
  # and scheduled emails.
  #
  # Pure loader — no side effects. The "visiting clears the nav dot" reminder stamp
  # stays in each controller, since it is a per-visit concern.
  class PageData
    AGENDA_LIMIT = 100 # how many upcoming items the agenda lists from the anchor date

    Result = Data.define(
      :calendar_accounts, :has_calendars, :managed_calendar_account_ids,
      :range, :prev_date, :next_date,
      :events, :reminders, :snoozed_threads, :scheduled_emails
    )

    # @param entitlements [#feature?] the caller's current_entitlements (gates the
    #   snoozed-thread / scheduled-email collections behind :email_scheduling)
    def self.for(user:, view:, date:, entitlements:)
      new(user:, view:, date:, entitlements:).call
    end

    def initialize(user:, view:, date:, entitlements:)
      @user = user
      @view = view.to_s
      @date = date
      @entitlements = entitlements
    end

    def call
      Result.new(
        calendar_accounts: calendar_accounts,
        has_calendars: calendar_accounts.any?,
        managed_calendar_account_ids: managed_calendar_account_ids,
        range: range,
        prev_date: prev_date,
        next_date: next_date,
        events: events,
        reminders: reminders,
        snoozed_threads: snoozed_threads,
        scheduled_emails: scheduled_emails
      )
    end

    private

    # Sidebar data: every readable account with its calendars, so the user can
    # show/hide, recolor, and enable more calendars without leaving the page.
    def calendar_accounts
      @calendar_accounts ||= @user.readable_calendar_accounts.active
                                  .includes(:calendars).order(:created_at)
    end

    # Which of those the user can manage (sidebar recolor / syncing affordances) —
    # one query instead of a permission check per account.
    def managed_calendar_account_ids
      @managed_calendar_account_ids ||=
        @user.calendar_account_users.where(can_manage: true).pluck(:calendar_account_id)
    end

    def range
      @range ||= range_for(@view, @date)
    end

    def adjacent
      @adjacent ||= adjacent_dates(@view, @date)
    end

    def prev_date = adjacent.first
    def next_date = adjacent.last

    def events
      # Concrete rows (plain events + provider-materialized instances) render as-is;
      # series masters (an app-created or Zoho series held as one row with an rrule)
      # are expanded into occurrences below. Agenda lists your next events from the
      # anchor forward (no hard window) so it never reads "nothing coming up" when
      # your next event is just past the month edge; week/month query their range.
      concrete = base_events.concrete.order(:start_at)
      scoped = if @view == "agenda"
        # Upcoming = from the anchor forward, minus timed events that already ended,
        # so the list never shows things that are over. All-day events stay.
        concrete.where(start_at: @date.beginning_of_day..)
                .where("COALESCE(calendar_events.end_at, calendar_events.start_at) >= :now OR calendar_events.all_day = :all_day",
                       now: Time.current, all_day: true)
                .limit(AGENDA_LIMIT)
      else
        concrete.in_range(range.begin, range.end)
      end

      # Fold each recurring series' occurrences into the window, letting a real synced
      # instance win over a locally-expanded ghost of the same slot.
      merged = Calendars::OccurrenceExpander.new(
        concrete: scoped, masters: base_events.series_masters, from: range.begin, to: range.end
      ).events
      @view == "agenda" ? merged.first(AGENDA_LIMIT) : merged
    end

    # Only calendars that are syncing render (off means off — matching the sidebar
    # checkboxes), minus the ones this user personally hid.
    def base_events
      base = CalendarEvent.accessible_to(@user).visible
                          .where(calendars: { syncing: true })
                          .includes(:event_type, calendar: :calendar_account)
      hidden_ids = Array(@user.hidden_calendar_ids)
      hidden_ids.any? ? base.where.not(calendar_id: hidden_ids) : base
    end

    # Pending reminders ride alongside events as distinct "suggestion" chips. Only
    # unconfirmed ones (confirmed reminders already exist as real CalendarEvents).
    # Empty until a calendar is connected, mirroring the classic page.
    def reminders
      return [] unless calendar_accounts.any?

      # Never surface past-due reminders — they're no longer actionable. Floor at
      # the start of today so all-day reminders due today still show.
      scope = Reminder.accessible_to(@user).pending.where(calendar_event_id: nil)
                      .where(due_at: Time.current.beginning_of_day..).order(:due_at)
      @view == "agenda" ? scope.limit(AGENDA_LIMIT) : scope.where(due_at: range.begin..range.end)
    end

    # Snoozed email threads shown on the calendar at their snooze-until time.
    def snoozed_threads
      return EmailThread.none unless @entitlements.feature?(:email_scheduling)

      scope = EmailThread.snoozed
                         .where(email_account: @user.readable_email_accounts)
                         .includes(:email_account)
      if @view == "agenda"
        scope.where(snoozed_until: @date.beginning_of_day..).order(:snoozed_until).limit(AGENDA_LIMIT)
      else
        scope.where(snoozed_until: range.begin..range.end)
      end
    end

    # Scheduled emails shown on the calendar at their send time.
    def scheduled_emails
      return ScheduledEmail.none unless @entitlements.feature?(:email_scheduling)

      scope = ScheduledEmail.accessible_to(@user).pending
      if @view == "agenda"
        scope.where("COALESCE(next_occurrence_at, scheduled_at) >= ?", @date.beginning_of_day)
             .order(Arel.sql("COALESCE(next_occurrence_at, scheduled_at) ASC")).limit(AGENDA_LIMIT)
      else
        scope.in_range(range.begin, range.end)
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
end
