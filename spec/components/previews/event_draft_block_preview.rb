# frozen_string_literal: true

class EventDraftBlockPreview < Lookbook::Preview
  # Draft state: extracted event awaiting user confirmation.
  # This is the initial block shown below the email body when a scheduling
  # proposal is detected. Scout drafts; the user taps Add.
  def draft
    render(Campbooks::EventDraftBlock.new(
      state:    :draft,
      title:    "Kickoff call with Dana",
      start_at: Time.zone.parse("2026-08-07 14:30"),
      end_at:   Time.zone.parse("2026-08-07 15:00"),
      edit_url: "#",
      add_url:  "#"
    ))
  end

  # Confirmed state: event was added to the calendar.
  # Replaces the draft block after a successful "Add to calendar" tap.
  # @label Confirmed — event added
  def confirmed
    event = CalendarEvent.new(
      id:       "00000000-0000-4000-8000-000000000001",
      title:    "Kickoff call with Dana",
      start_at: Time.zone.parse("2026-08-07 14:30"),
      end_at:   Time.zone.parse("2026-08-07 15:00"),
      status:   :confirmed
    )
    render(Campbooks::EventDraftBlock.new(
      state: :confirmed,
      event: event
    ))
  end

  # Error state: creation failed.
  # Rendered when Tools::CreateCalendarEvent returns a failure (e.g. no
  # calendar reachable). Shows the error message and a Retry button.
  def error
    render(Campbooks::EventDraftBlock.new(
      state:         :error,
      error_message: "No calendar available. Connect a calendar in Settings.",
      add_url:       "#"
    ))
  end
end
