# frozen_string_literal: true

# Previews for one Time-agenda row. In-memory records (ids set so the
# keep/move/dismiss/confirm/complete route helpers resolve) stand behind each
# Time::AgendaItem kind.
class TimeAgendaRowComponentPreview < ViewComponent::Preview
  # A calendar event with a video link → the "Meet" chip.
  def event_with_meet
    render Campbooks::TimePage::AgendaRow.new(item: event_item, zone: zone)
  end

  # A deadline Scout found in mail → "Open thread" + an "Add to calendar" kebab.
  def deadline
    render Campbooks::TimePage::AgendaRow.new(item: deadline_item, zone: zone)
  end

  # A past-due deadline pinned into today → the red "overdue" badge.
  def overdue_deadline
    render Campbooks::TimePage::AgendaRow.new(item: deadline_item(overdue: true), zone: zone)
  end

  # A due-dated task → the "Done" action.
  def task
    render Campbooks::TimePage::AgendaRow.new(item: task_item, zone: zone)
  end

  # A proposed focus block → Move (with a slot popover), Keep, and a dismiss kebab.
  def focus
    render Campbooks::TimePage::AgendaRow.new(item: focus_item, zone: zone,
      move_slots: [ at(9), at(11), at(12) ])
  end

  private

  def zone = ActiveSupport::TimeZone["Europe/Lisbon"]
  def at(hour, min = 0) = zone.now.change(hour: hour, min: min)

  def event_item
    Time::AgendaItem.new(
      kind: :event, at: at(9, 30), day: Date.current, all_day: false, overdue: false,
      duration_minutes: 30, title: "Standup", source_label: "Google Calendar", source_path: nil,
      color: "#2563eb", record: CalendarEvent.new(id: SecureRandom.uuid, conference_url: "https://meet.google.com/abc-defg-hij"),
      actions: [ :meet ]
    )
  end

  def deadline_item(overdue: false)
    Time::AgendaItem.new(
      kind: :deadline, at: at(11), day: Date.current, all_day: false, overdue: overdue,
      duration_minutes: 0, title: "Reply to Ines about the notice period",
      source_label: "from Ines's email", source_path: "/email_messages/1",
      color: nil, record: Reminder.new(id: SecureRandom.uuid), actions: [ :open_thread, :add_to_calendar ]
    )
  end

  def task_item
    Time::AgendaItem.new(
      kind: :task, at: at(16), day: Date.current, all_day: false, overdue: false,
      duration_minutes: 0, title: "Review the Q3 budget", source_label: "Scout suggested",
      source_path: nil, color: nil, record: Task.new(id: SecureRandom.uuid), actions: [ :done ]
    )
  end

  def focus_item
    Time::AgendaItem.new(
      kind: :focus, at: at(10), day: Date.current + 1, all_day: false, overdue: false,
      duration_minutes: 45, title: "Focus: Q3 deck comments", source_label: "Scout suggested",
      source_path: nil, color: nil, record: FocusBlock.new(id: SecureRandom.uuid), actions: [ :move, :keep, :dismiss_focus ]
    )
  end
end
