# frozen_string_literal: true

# Previews for the Time agenda list — day sections over a mix of Time::AgendaItem
# kinds, plus the empty (today only) state.
class TimeAgendaListComponentPreview < ViewComponent::Preview
  # A full day: an event, a deadline, a dated ask today; a proposed focus tomorrow;
  # plus a "No date yet" section with two undated asks.
  def full
    rows = items
    focus = rows.find(&:focus?)
    render Campbooks::TimePage::AgendaList.new(items: rows, zone: zone, undated: undated_items,
      move_slots: { focus.record.id => [ at(9, day: Date.current + 1), at(11, day: Date.current + 1) ] })
  end

  # Nothing scheduled — only today's section renders, with the empty line.
  def empty
    render Campbooks::TimePage::AgendaList.new(items: [], zone: zone)
  end

  private

  def zone = ActiveSupport::TimeZone["Europe/Lisbon"]
  def at(hour, min = 0, day: Date.current) = zone.local(day.year, day.month, day.day, hour, min)

  def items
    [
      Time::AgendaItem.new(kind: :event, at: at(9, 30), day: Date.current, all_day: false, overdue: false,
        duration_minutes: 30, title: "Standup", source_label: "Google Calendar", source_path: nil,
        color: "#2563eb", record: CalendarEvent.new(id: SecureRandom.uuid, conference_url: "https://meet.google.com/abc"), actions: [ :meet ]),
      Time::AgendaItem.new(kind: :deadline, at: at(11), day: Date.current, all_day: false, overdue: false,
        duration_minutes: 0, title: "Reply to Ines about the notice period", source_label: "from Ines's email",
        source_path: "/email_messages/1", color: nil, record: Reminder.new(id: SecureRandom.uuid), actions: [ :open_thread, :add_to_calendar ]),
      Time::AgendaItem.new(kind: :task, at: at(9, 30), day: Date.current, all_day: false, overdue: false,
        duration_minutes: 0, title: "Comments to Sofia on slides 4 to 9",
        source_label: "from Sofia's email · held Thu 10:00", source_path: "/email_messages/1",
        color: nil, record: Task.new(id: SecureRandom.uuid), actions: [ :open_thread, :done, :change_date, :snooze ]),
      Time::AgendaItem.new(kind: :focus, at: at(10, 0, day: Date.current + 1), day: Date.current + 1, all_day: false,
        overdue: false, duration_minutes: 45, title: "Focus: Q3 deck comments", source_label: "Scout suggested",
        source_path: nil, color: nil, record: FocusBlock.new(id: SecureRandom.uuid), actions: [ :move, :keep, :dismiss_focus ])
    ]
  end

  def undated_items
    [
      Time::AgendaItem.new(kind: :task, at: nil, day: nil, all_day: true, overdue: false, duration_minutes: 0,
        title: "Send the signed contract back to Acme", source_label: "from Rita's email",
        source_path: "/email_messages/2", color: nil, record: Task.new(id: SecureRandom.uuid),
        actions: [ :open_thread, :schedule, :hold, :done, :snooze ]),
      Time::AgendaItem.new(kind: :task, at: nil, day: nil, all_day: true, overdue: false, duration_minutes: 0,
        title: "Confirm the office move date with Marta", source_label: "from Marta's email",
        source_path: "/email_messages/3", color: nil, record: Task.new(id: SecureRandom.uuid),
        actions: [ :open_thread, :schedule, :hold, :done, :snooze ])
    ]
  end
end
