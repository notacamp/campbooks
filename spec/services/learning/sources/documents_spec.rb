# frozen_string_literal: true

require "rails_helper"

RSpec.describe Learning::Sources::Documents, type: :service do
  let(:workspace) { create(:workspace) }

  def build_doc(**attrs)
    doc = workspace.documents.new(
      document_type: "expense_invoice", source: :manual_upload,
      ai_status: :completed, review_status: :approved,
      **attrs
    )
    doc.original_file.attach(io: StringIO.new("x"), filename: "invoice.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end

  let(:document_type) { create(:document_type, workspace: workspace, name: "Invoice", category: "accounting") }

  describe "#tally_for(:sender)" do
    it "returns tally of document_type_ids for documents from the same sender" do
      subject_doc = build_doc(sender_name: "supplier@example.com")
      other = build_doc(sender_name: "supplier@example.com", review_status: :approved)
      other.update_columns(document_type_id: document_type.id)
      unrelated = build_doc(sender_name: "other@example.com")
      unrelated.update_columns(document_type_id: document_type.id)

      source = described_class.new(subject_doc)
      tally = source.tally_for(:sender)
      expect(tally).to include(document_type.id)
    end

    it "uses LOWER() for case-insensitive sender match via metadata" do
      subject_doc = build_doc(sender_name: "Supplier@Example.COM")
      peer = build_doc(sender_name: "supplier@example.com", review_status: :approved)
      peer.update_columns(document_type_id: document_type.id)

      source = described_class.new(subject_doc)
      tally = source.tally_for(:sender)
      expect(tally).to include(document_type.id)
    end

    it "excludes the subject document from its own corpus" do
      doc = build_doc(sender_name: "solo@example.com")
      doc.update_columns(document_type_id: document_type.id)

      source = described_class.new(doc)
      tally = source.tally_for(:sender)
      # The only sender-matched doc IS the subject — should not appear in tally
      expect(tally).to be_nil.or be_empty
    end
  end
end
