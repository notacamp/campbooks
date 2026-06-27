# frozen_string_literal: true

class PipelineBoardColumnComponentPreview < ViewComponent::Preview
  # A normal droppable column with several document cards.
  def with_items
    render Campbooks::Pipeline::BoardColumn.new(column: {
      stage: stage_stub,
      memberships: memberships_sample(3),
      has_more: false,
      draggable: true
    })
  end

  # An empty column — shows the empty-state placeholder text.
  def empty
    render Campbooks::Pipeline::BoardColumn.new(column: {
      stage: stage_stub(name: "Approved", color: "#8b5cf6"),
      memberships: [],
      has_more: false,
      draggable: true
    })
  end

  # A terminal stage column — read-only (lock badge in header, cards cannot be
  # dragged out); items can still be dropped in.
  def terminal
    render Campbooks::Pipeline::BoardColumn.new(column: {
      stage: stage_stub(name: "Done", color: "#10b981", is_terminal: true),
      memberships: memberships_sample(2),
      has_more: false,
      draggable: false
    })
  end

  private

  def pipeline_stub
    Pipeline.new(id: 1, name: "Invoices", applies_to: :both)
  end

  def stage_stub(name: "To review", color: "#6366f1", is_terminal: false)
    stage = PipelineStage.new(id: 1, name: name, color: color, position: 1, is_terminal: is_terminal)
    # BoardColumn reads @stage.pipeline to pass to child BoardCards.
    stage.pipeline = pipeline_stub
    stage
  end

  def memberships_sample(count)
    titles = [ "ACME invoice.pdf", "NDA Agreement.pdf", "PO #2025-889.pdf" ]
    Array.new(count) do |i|
      title = titles[i % titles.size]
      doc = Document.new(id: i + 1, created_at: (i + 1).hours.ago)
      doc.define_singleton_method(:display_title) { title }
      doc.define_singleton_method(:classification) { nil }
      m = PipelineMembership.new(id: i + 1, current_stage_id: 1)
      m.item = doc
      m
    end
  end
end
