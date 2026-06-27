# frozen_string_literal: true

# Previews for the calendar views — agenda list, month + week grids, and the
# agenda event row. In-memory records (ids set so the event route helpers
# resolve) stand in for synced events; nothing is persisted.
class CalendarPreview < ViewComponent::Preview
  def agenda_list
    render Campbooks::Calendar::AgendaList.new(events: sample_events)
  end

  def agenda_empty
    render Campbooks::Calendar::AgendaList.new(events: [])
  end

  def month_grid
    render Campbooks::Calendar::MonthGrid.new(date: Date.current, events: sample_events)
  end

  def week_grid
    render Campbooks::Calendar::WeekGrid.new(date: Date.current, events: sample_events)
  end

  def week_time_grid
    base = Date.current.to_time
    events = [
      event(id: 31, title: "Standup", start_at: base.change(hour: 9)),
      event(id: 32, title: "Design review", start_at: base.change(hour: 9, min: 30)),
      event(id: 33, title: "1:1 with Sam", start_at: (base + 86_400).change(hour: 14)),
      event(id: 35, title: "Offsite", start_at: (base + 3 * 86_400), all_day: true)
    ]
    render Campbooks::Calendar::WeekTimeGrid.new(date: Date.current, events: events)
  end

  def day_grid
    base = Date.current.to_time
    events = [
      event(id: 21, title: "All-hands", start_at: base.change(hour: 10)),
      event(id: 22, title: "Design sync", start_at: base.change(hour: 10, min: 30)),
      event(id: 23, title: "Lunch with Jamie", start_at: base.change(hour: 12, min: 30), location: "Cafe"),
      event(id: 24, title: "Offsite", start_at: base, all_day: true)
    ]
    render Campbooks::Calendar::DayGrid.new(date: Date.current, events: events)
  end

  def event_row
    render Campbooks::Calendar::EventRow.new(event: event(id: 1, title: "Design review", start_at: Time.current.change(hour: 14), location: "Room 2"))
  end

  def event_row_all_day
    render Campbooks::Calendar::EventRow.new(event: event(id: 2, title: "Company offsite", start_at: Date.current.to_time, all_day: true))
  end

  def event_row_recurring
    render Campbooks::Calendar::EventRow.new(event: event(id: 3, title: "Weekly standup", start_at: Time.current.change(hour: 9), recurring: true))
  end

  # Snoozed email threads on the calendar (purple). The chip is the grid cell
  # form; the row is the agenda-list form.
  def snoozed_chip
    render Campbooks::Calendar::SnoozedChip.new(thread: snoozed_thread)
  end

  def snoozed_row
    render Campbooks::Calendar::SnoozedRow.new(thread: snoozed_thread)
  end

  # Scheduled outbound emails on the calendar (cyan). The row shows the recurring
  # glyph when the schedule repeats.
  def scheduled_email_chip
    render Campbooks::Calendar::ScheduledEmailChip.new(scheduled_email: scheduled_mail)
  end

  def scheduled_email_row
    render Campbooks::Calendar::ScheduledEmailRow.new(scheduled_email: scheduled_mail(id: 52, subject: "Weekly status update", rrule: "FREQ=WEEKLY"))
  end

  private

  def snoozed_thread(id: 41, subject: "Re: Q3 budget approval")
    EmailThread.new(
      id: id, subject: subject,
      snoozed_until: Date.current.to_time.change(hour: 14),
      email_account: EmailAccount.new(id: 1, name: "Personal", email_address: "me@example.com")
    )
  end

  def scheduled_mail(id: 51, subject: "Reminder: send invoice", rrule: nil)
    ScheduledEmail.new(
      id: id, subject: subject, to_address: "Riley Chen <riley@client.com>",
      scheduled_at: Date.current.to_time.change(hour: 16), rrule: rrule
    )
  end

  def calendar(color: "#2ea55c")
    Calendar.new(id: 1, name: "Personal", color: color, calendar_account: CalendarAccount.new(id: 1, color: color))
  end

  def event(id:, title:, start_at:, all_day: false, location: nil, recurring: false)
    CalendarEvent.new(
      id: id, title: title, start_at: start_at, end_at: start_at + 3600,
      all_day: all_day, location: location, status: :confirmed,
      recurring_event_provider_id: (recurring ? "series-1" : nil),
      calendar: calendar
    )
  end

  def sample_events
    base = Date.current.to_time
    [
      event(id: 11, title: "Standup", start_at: base.change(hour: 9)),
      event(id: 12, title: "Design review", start_at: base.change(hour: 14), location: "Room 2"),
      event(id: 13, title: "1:1 with Sam", start_at: (base + 86_400).change(hour: 11)),
      event(id: 14, title: "Client call", start_at: (base + 3 * 86_400).change(hour: 16), recurring: true)
    ]
  end
end
