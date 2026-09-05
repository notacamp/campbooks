# frozen_string_literal: true

# @label Hint Tip
class HintTipPreview < Lookbook::Preview
  # The shared tooltip element rendered once in the layout. In production it is
  # driven by hint_controller.js and is invisible until a hinted control is
  # hovered. The previews below simulate the visible states.

  # @label Default (label only)
  def label_only
    render Campbooks::HintTip.new(preview: { label: "Star", keys: [] })
  end

  # @label With single key
  def single_key
    render Campbooks::HintTip.new(preview: { label: "Reply", keys: %w[R] })
  end

  # @label With modifier key
  def modifier_key
    # A modifier combo is ONE cap ("⇧U"); only a sequence ("g p") splits into two.
    render Campbooks::HintTip.new(preview: { label: "Mark unread", keys: %w[⇧U] })
  end

  # @label With sequence (two caps)
  def key_sequence
    render Campbooks::HintTip.new(preview: { label: "People", keys: %w[G P] })
  end

  # @label Hinted controls
  # @display background_color "#f5f5f4"
  def hinted_controls
    render_with_template
  end
end
