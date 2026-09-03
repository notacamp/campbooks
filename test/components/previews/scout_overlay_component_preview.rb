# @label Scout Overlay
class ScoutOverlayComponentPreview < Lookbook::Preview
  # Browse: the query line + suggestions + Recent + the command list.
  def browse
    render Campbooks::ScoutOverlay.new(preview: :browse)
  end

  # Conversation: Scout's reply in place, the input moved to the foot.
  def conversation
    render Campbooks::ScoutOverlay.new(preview: :conversation)
  end

  # The live shell as mounted in the layout (an empty lazy frame — the body loads
  # from /scout/overlay on first open).
  def shell
    render Campbooks::ScoutOverlay.new
  end
end
