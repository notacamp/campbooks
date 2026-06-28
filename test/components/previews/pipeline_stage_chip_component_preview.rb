# frozen_string_literal: true

class PipelineStageChipComponentPreview < ViewComponent::Preview
  # A chip for a regular (non-terminal) pipeline stage.
  def default
    render Campbooks::Pipeline::StageChip.new(stage: stage_stub)
  end

  # A chip for a terminal stage — rendered with a checkmark glyph alongside the
  # stage name.
  def terminal
    render Campbooks::Pipeline::StageChip.new(stage: stage_stub(name: "Done", color: "#10b981", is_terminal: true))
  end

  private

  def stage_stub(name: "To review", color: "#6366f1", is_terminal: false)
    PipelineStage.new(id: 1, name: name, color: color, position: 1, is_terminal: is_terminal)
  end
end
