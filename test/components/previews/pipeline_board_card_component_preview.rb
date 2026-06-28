# frozen_string_literal: true

class PipelineBoardCardComponentPreview < ViewComponent::Preview
  # A draggable board card backed by a Document item.
  def document
    render Campbooks::Pipeline::BoardCard.new(membership: document_membership, pipeline: pipeline_stub)
  end

  # A draggable board card backed by an EmailMessage item.
  def email
    render Campbooks::Pipeline::BoardCard.new(membership: email_membership, pipeline: pipeline_stub)
  end

  # A read-only card (draggable: false) shown in a terminal stage — no grab
  # cursor, cannot be dragged to another column.
  def read_only
    render Campbooks::Pipeline::BoardCard.new(membership: document_membership, pipeline: pipeline_stub, draggable: false)
  end

  private

  def pipeline_stub
    Pipeline.new(id: 1, name: "Invoices", applies_to: :both)
  end

  # Build an unsaved Document + PipelineMembership pair. display_title and
  # classification are stubbed via singleton methods to avoid any file/DB access.
  def document_membership
    doc = Document.new(id: 1, created_at: 2.hours.ago)
    doc.define_singleton_method(:display_title) { "ACME invoice.pdf" }
    doc.define_singleton_method(:classification) { nil }
    m = PipelineMembership.new(id: 1, current_stage_id: 1)
    m.item = doc
    m
  end

  def email_membership
    email = EmailMessage.new(
      id: 2,
      subject: "Re: contract",
      from_address: "sam@example.com",
      received_at: 3.hours.ago
    )
    m = PipelineMembership.new(id: 2, current_stage_id: 1)
    m.item = email
    m
  end
end
