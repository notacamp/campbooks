# frozen_string_literal: true

# Loads the bold Time agenda (the merged Time::AgendaItem list + the undated asks +
# the Move popover's slot options + the ask "Hold" slot previews + the
# snoozed/scheduled tail), anchored on a date. Shared by TimeController (the page),
# FocusBlocksController (which re-renders the agenda after Keep / Move / dismiss)
# and AsksController (after an ask action).
module TimeAgendaLoading
  extend ActiveSupport::Concern

  AGENDA_DAYS = 30

  included do
    # The shared AgendaList builder is used from the Time page view too.
    helper_method :agenda_list
  end

  def load_time_agenda(date: Date.current)
    @date = date
    @zone = current_user.effective_time_zone

    data = Calendars::PageData.for(user: current_user, view: "agenda", date: @date, entitlements: current_entitlements)
    @calendar_accounts = data.calendar_accounts
    @has_calendars = data.has_calendars
    @snoozed_threads = data.snoozed_threads
    @scheduled_emails = data.scheduled_emails

    from = @zone.parse(@date.iso8601).beginning_of_day
    to = @zone.parse((@date + AGENDA_DAYS.days).iso8601).end_of_day
    @agenda = Time::Agenda.for(current_user, from:, to:)
    @undated = Time::Agenda.undated_for(current_user)
    @move_slots = agenda_move_slots(@agenda)
    @hold_slots = agenda_hold_slots(@agenda + @undated)
  end

  # For each proposed/moved focus row, the next few free slots the Move popover
  # offers — keyed by focus-block id.
  def agenda_move_slots(agenda)
    agenda.select(&:focus?).each_with_object({}) do |item, memo|
      memo[item.record.id] = Time::SlotSuggester.alternatives(item.record, user: current_user)
    end
  end

  # Preview the "Hold" slot for every held-less ask row (dated kebab + undated
  # button), keyed by ask id — so the row can label the control "Hold Thu 10:00".
  # Time::BusyIntervals memoizes per window, so the whole deck costs one calendar
  # query per window rather than one per row.
  def agenda_hold_slots(items)
    items.select { |item| item.task? && item.action?(:hold) }.each_with_object({}) do |item, memo|
      memo[item.record.id] = Time::FocusHolder.preview(item.record, user: current_user)
    end
  end

  # The one AgendaList builder every Time surface renders through, so the page, the
  # focus actions and the ask actions never drift. Reads the ivars set by
  # #load_time_agenda (or, on the page, by TimeController#index).
  def agenda_list
    Campbooks::TimePage::AgendaList.new(
      items: @agenda, undated: @undated,
      move_slots: @move_slots, hold_slots: @hold_slots,
      snoozed_threads: @snoozed_threads, scheduled_emails: @scheduled_emails,
      zone: @zone, has_calendars: @has_calendars
    )
  end
end
