# frozen_string_literal: true

# Previews for Scout's day note (the scout-glass card). In-memory Time::DayNote
# results stand in for the real (pure) reader.
class TimeDayNoteComponentPreview < ViewComponent::Preview
  # Meetings + a deadline + a held focus block — the full three-clause note.
  def full
    render Campbooks::TimePage::DayNote.new(note: note, zone: zone)
  end

  # Meetings only.
  def meetings_only
    render Campbooks::TimePage::DayNote.new(
      note: ::Time::DayNote::Result.new(date: Date.current, meetings_count: 3, deadlines_count: 0,
        first_deadline_title: nil, focus: nil, late_obligation: nil, undated_count: 0),
      zone: zone
    )
  end

  # Two undated asks and nothing else — just the "N asks have no date yet" clause.
  def undated_only
    render Campbooks::TimePage::DayNote.new(
      note: ::Time::DayNote::Result.new(date: Date.current, meetings_count: 0, deadlines_count: 0,
        first_deadline_title: nil, focus: nil, late_obligation: nil, undated_count: 2),
      zone: zone
    )
  end

  # An empty day — the "clear" fallback line.
  def clear_day
    render Campbooks::TimePage::DayNote.new(
      note: ::Time::DayNote::Result.new(date: Date.current, meetings_count: 0, deadlines_count: 0,
        first_deadline_title: nil, focus: nil, late_obligation: nil, undated_count: 0),
      zone: zone
    )
  end

  private

  def zone = ActiveSupport::TimeZone["Europe/Lisbon"]

  def note
    focus = ::Time::DayNote::Focus.new(
      title: "Focus: Q3 deck comments", subject: "Q3 deck comments",
      at: zone.now.change(hour: 10, min: 0) + 1.day, duration_minutes: 45
    )
    ::Time::DayNote::Result.new(date: Date.current, meetings_count: 2, deadlines_count: 1,
      first_deadline_title: "Reply to Ines about the notice period", focus: focus,
      late_obligation: nil, undated_count: 2)
  end
end
