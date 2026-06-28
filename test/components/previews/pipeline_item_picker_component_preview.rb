# frozen_string_literal: true

class PipelineItemPickerComponentPreview < ViewComponent::Preview
  # The picker loaded with a mix of document and email results (blank query).
  def with_items
    render Campbooks::Pipeline::ItemPicker.new(
      pipeline: pipeline_stub,
      items: sample_items,
      query: ""
    )
  end

  # The picker in its empty state — no results matched the search term.
  def empty
    render Campbooks::Pipeline::ItemPicker.new(
      pipeline: pipeline_stub,
      items: [],
      query: "zzz"
    )
  end

  private

  def pipeline_stub
    Pipeline.new(id: 1, name: "Invoices", applies_to: :both)
  end

  # A small set of unsaved items (two documents + one email) that exercise
  # both branches of item_title / item_subtitle inside ItemPicker.
  def sample_items
    doc1 = Document.new(id: 1, created_at: 1.hour.ago)
    doc1.define_singleton_method(:display_title) { "ACME invoice.pdf" }
    doc1.define_singleton_method(:classification) { nil }

    doc2 = Document.new(id: 2, created_at: 2.hours.ago)
    doc2.define_singleton_method(:display_title) { "NDA Agreement.pdf" }
    doc2.define_singleton_method(:classification) { nil }

    email = EmailMessage.new(
      id: 3,
      subject: "Re: contract",
      from_address: "sam@example.com",
      received_at: 3.hours.ago
    )

    [ doc1, doc2, email ]
  end
end
