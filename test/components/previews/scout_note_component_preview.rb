# frozen_string_literal: true

class ScoutNoteComponentPreview < ViewComponent::Preview
  # Scout's contribution block (light Ember glass) with a read + timestamp.
  def default
    render Campbooks::ScoutNote.new(
      message: "matches your approved March quote, nothing unusual. I drafted an approval reply.",
      time: "read it just now"
    )
  end

  # Without the timestamp (e.g. inside the skim overlay).
  def without_time
    render Campbooks::ScoutNote.new(
      message: "I filed this to your March receipts. Nothing here needs you."
    )
  end

  # Compact one-line shape used on the dense home feed: Ember spark + "Scout" +
  # the read, clamped to a single line.
  def compact
    render Campbooks::ScoutNote.new(
      message: "matches your approved March quote, nothing unusual — I drafted an approval reply.",
      compact: true
    )
  end
end
