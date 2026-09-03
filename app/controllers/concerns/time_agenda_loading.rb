# frozen_string_literal: true

# Loads the bold Time agenda (the merged Time::AgendaItem list + the Move
# popover's slot options + the snoozed/scheduled tail), anchored on a date. Shared
# by TimeController (the page) and FocusBlocksController (which re-renders the
# agenda in place after Keep / Move / dismiss).
module TimeAgendaLoading
  extend ActiveSupport::Concern

  AGENDA_DAYS = 30

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
    @move_slots = agenda_move_slots(@agenda)
  end

  # For each proposed/moved focus row, the next few free slots the Move popover
  # offers — keyed by focus-block id.
  def agenda_move_slots(agenda)
    agenda.select(&:focus?).each_with_object({}) do |item, memo|
      memo[item.record.id] = Time::SlotSuggester.alternatives(item.record, user: current_user)
    end
  end
end
