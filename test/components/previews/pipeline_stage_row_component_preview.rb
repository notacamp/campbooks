# frozen_string_literal: true

class PipelineStageRowComponentPreview < ViewComponent::Preview
  # An existing stage row pre-filled with name, description, and colour; used
  # when editing a persisted pipeline.
  def existing
    stage = PipelineStage.new(
      id: 1,
      name: "To review",
      description: "Needs human sign-off",
      color: "#6366f1",
      position: 1,
      is_terminal: false
    )
    render Campbooks::Pipeline::StageRow.new(index: 0, stage: stage)
  end

  # The blank template row (index "NEW_RECORD", stage nil) that the
  # pipeline-builder Stimulus controller clones when "Add stage" is clicked.
  def blank
    render Campbooks::Pipeline::StageRow.new(index: "NEW_RECORD", stage: nil)
  end
end
