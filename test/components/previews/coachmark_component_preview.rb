# frozen_string_literal: true

# The coachmark is positioned by its Stimulus controller against an on-page anchor,
# so the preview renders a mock target (three "rings") for it to point at. Top-level
# class name to match the file path (Zeitwerk).
class CoachmarkComponentPreview < ViewComponent::Preview
  # @label On a target (points at the mock rings)
  def default
    render_with_template
  end

  # @label Above a single target (placement: top)
  def above
    render_with_template
  end
end
