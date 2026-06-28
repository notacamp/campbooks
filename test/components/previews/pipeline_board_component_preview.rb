# frozen_string_literal: true

class PipelineBoardComponentPreview < ViewComponent::Preview
  # A fully populated board with two columns (one regular, one terminal) and
  # document cards in each.
  def populated
    pipeline = pipeline_stub
    render Campbooks::Pipeline::Board.new(pipeline: pipeline, columns: columns_sample(pipeline))
  end

  # An empty board — no stages have been added yet; shows the "Add stages"
  # empty-state prompt.
  def no_stages
    render Campbooks::Pipeline::Board.new(pipeline: pipeline_stub, columns: [])
  end

  private

  def pipeline_stub
    Pipeline.new(id: 1, name: "Invoices", applies_to: :both)
  end

  def stage(id:, name:, color:, is_terminal: false, pipeline:)
    s = PipelineStage.new(id: id, name: name, color: color, position: id, is_terminal: is_terminal)
    # Board passes @stage.pipeline down to each BoardColumn, which passes it on
    # to each BoardCard so the remove-form action URL resolves correctly.
    s.pipeline = pipeline
    s
  end

  def doc_membership(id:, stage_id:)
    doc = Document.new(id: id, created_at: id.hours.ago)
    doc.define_singleton_method(:display_title) { "Invoice ##{id}.pdf" }
    doc.define_singleton_method(:classification) { nil }
    m = PipelineMembership.new(id: id, current_stage_id: stage_id)
    m.item = doc
    m
  end

  def columns_sample(pipeline)
    review = stage(id: 1, name: "To review", color: "#6366f1", pipeline: pipeline)
    done   = stage(id: 2, name: "Done", color: "#10b981", is_terminal: true, pipeline: pipeline)
    [
      {
        stage: review,
        memberships: [ doc_membership(id: 1, stage_id: 1), doc_membership(id: 2, stage_id: 1) ],
        has_more: false,
        draggable: true
      },
      {
        stage: done,
        memberships: [ doc_membership(id: 3, stage_id: 2) ],
        has_more: false,
        draggable: false
      }
    ]
  end
end
