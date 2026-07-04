# frozen_string_literal: true

# Previews for the calendar views — agenda list, month + week grids, and the
# agenda event row. In-memory records (ids set so the event route helpers
# resolve) stand in for synced events; nothing is persisted.
class CalendarPreview < ViewComponent::Preview
  def agenda_list
    render Campbooks::Calendar::AgendaList.new(events: sample_events, reminders: sample_reminders)
  end

  def agenda_empty
    render Campbooks::Calendar::AgendaList.new(events: [])
  end

  def month_grid
    render Campbooks::Calendar::MonthGrid.new(date: Date.current, events: sample_events)
  end

  # A day with more events than fit — exercises the "+N more" chip / mobile dots
  # and the day popover that expands to the full list.
  def month_grid_overflow
    base = Date.current.to_time
    titles = [ "Standup", "Design review", "1:1 with Sam", "Client call", "Lunch with Jamie", "Team retro" ]
    events = titles.each_with_index.map do |title, i|
      event(id: 70 + i, title: title, start_at: base.change(hour: 9 + i))
    end
    render Campbooks::Calendar::MonthGrid.new(date: Date.current, events: events)
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

  # The calendar-management sidebar: calendars grouped by owning account, with
  # per-user show/hide checkboxes. The manager variant adds the "⋯" color/sync
  # menus, the add-calendars disclosure, and the list refresh.
  def sidebar_as_manager
    render Campbooks::Calendar::Sidebar.new(
      accounts: sidebar_accounts, user: sidebar_user, view: "month", date: Date.current,
      managed_account_ids: [ uuid_for(101) ]
    )
  end

  def sidebar_as_viewer
    render Campbooks::Calendar::Sidebar.new(
      accounts: sidebar_accounts, user: sidebar_user, view: "month", date: Date.current
    )
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
      # calendar_events has a uuid primary key; a bare integer casts to nil and
      # breaks edit_calendar_event_path, so seed a deterministic valid uuid.
      id: uuid_for(id), title: title, start_at: start_at, end_at: start_at + 3600,
      all_day: all_day, location: location, status: :confirmed,
      recurring_event_provider_id: (recurring ? "series-1" : nil),
      calendar: calendar
    )
  end

  def uuid_for(n)
    "00000000-0000-4000-8000-#{n.to_s.rjust(12, '0')}"
  end

  # Two in-memory accounts for the sidebar: a work account with a mix of synced,
  # unsynced, and (for the sample user) one hidden calendar, plus a personal one.
  def sidebar_accounts
    work = CalendarAccount.new(id: uuid_for(101), email_address: "work@example.com", name: "Work", color: "#595dec")
    personal = CalendarAccount.new(id: uuid_for(102), email_address: "me@example.com", color: "#e76e08")
    work.calendars = [
      Calendar.new(id: uuid_for(111), name: "Work calendar", is_primary: true, syncing: true, calendar_account: work),
      Calendar.new(id: uuid_for(112), name: "Team offsites", color: "#00a8a8", syncing: true, calendar_account: work),
      Calendar.new(id: uuid_for(113), name: "Deploys", color: "#de3b3d", syncing: true, calendar_account: work),
      Calendar.new(id: uuid_for(114), name: "Public holidays", color: "#2ea55c", syncing: false, calendar_account: work)
    ]
    personal.calendars = [
      Calendar.new(id: uuid_for(121), name: "Personal", is_primary: true, syncing: true, calendar_account: personal)
    ]
    [ work, personal ]
  end

  # The sample viewer has hidden "Deploys" — its row renders unchecked/muted.
  def sidebar_user
    User.new(hidden_calendar_ids: [ uuid_for(113) ])
  end

  # Relative to now so the agenda countdown always reads sensibly (In 40 min ·
  # In 3 h · Tomorrow · In 3 days), whatever time of day the preview is opened.
  def sample_events
    now = Time.current
    [
      event(id: 11, title: "Standup", start_at: now + 40.minutes),
      event(id: 12, title: "Design review", start_at: now + 3.hours, location: "Room 2"),
      event(id: 13, title: "1:1 with Sam", start_at: (now + 1.day).change(hour: 11)),
      event(id: 14, title: "Client call", start_at: (now + 3.days).change(hour: 16), recurring: true)
    ]
  end

  def sample_reminders
    now = Time.current
    [
      Reminder.new(id: 61, title: "Pay invoice #1042", due_at: (now + 2.days).change(hour: 17),
                   all_day: false, reminder_type: :payment_due, status: :pending),
      Reminder.new(id: 62, title: "Passport renewal", due_at: (now + 9.days).beginning_of_day, all_day: true,
                   reminder_type: :renewal, status: :pending)
    ]
  end
end
