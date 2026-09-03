# frozen_string_literal: true

# Previews for the calendar FocusChip — a proposed focus block on the grids, in its
# compact chip and agenda-row shapes.
class FocusChipComponentPreview < ViewComponent::Preview
  # The compact pill used in month/week cells.
  def chip
    render Campbooks::Calendar::FocusChip.new(focus_block: block)
  end

  # The agenda-row shape.
  def row
    render Campbooks::Calendar::FocusChip.new(focus_block: block, variant: :row)
  end

  private

  def block
    start_at = Time.current.change(hour: 10, min: 0) + 1.day
    FocusBlock.new(id: SecureRandom.uuid, title: "Focus: Q3 deck comments", start_at: start_at, end_at: start_at + 45.minutes,
                   status: "proposed")
  end
end
