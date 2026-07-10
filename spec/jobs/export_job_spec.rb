# frozen_string_literal: true

require "rails_helper"

RSpec.describe ExportJob, type: :job do
  let(:workspace) { create(:workspace) }

  def build_doc(ai_status: :completed, review_status: :pending, **attrs)
    doc = workspace.documents.new(
      document_type: "other", source: :manual_upload,
      ai_status: ai_status, review_status: review_status,
      **attrs
    )
    doc.original_file.attach(
      io: StringIO.new("fake pdf content"),
      filename: "doc.pdf",
      content_type: "application/pdf"
    )
    doc.save!
    doc
  end

  def create_export(filters = {})
    workspace.exports.create!(status: :pending, filters: filters)
  end

  # Stub the zip generator so we don't need real PDF content.
  before do
    allow(Exports::ZipGenerator).to receive(:new).and_return(
      double(call: "FAKE_ZIP_CONTENT")
    )
    allow(Notifier).to receive(:export_ready)
    allow(Notifier).to receive(:export_failed)
  end

  # ── base scope: no failed AI, no rejected ────────────────────────────────

  describe "base scope" do
    it "excludes documents where ai_status is not completed" do
      build_doc(ai_status: :pending)
      build_doc(ai_status: :failed)
      build_doc(ai_status: :processing)
      completed = build_doc(ai_status: :completed, review_status: :pending)

      export = create_export
      described_class.perform_now(export.id)

      expect(export.reload.documents_count).to eq(1)
    end

    it "excludes review_rejected documents" do
      build_doc(ai_status: :completed, review_status: :rejected)
      kept = build_doc(ai_status: :completed, review_status: :approved)

      export = create_export
      described_class.perform_now(export.id)

      expect(export.reload.documents_count).to eq(1)
    end

    it "includes both pending and approved documents" do
      build_doc(ai_status: :completed, review_status: :pending)
      build_doc(ai_status: :completed, review_status: :approved)

      export = create_export
      described_class.perform_now(export.id)

      expect(export.reload.documents_count).to eq(2)
    end

    it "does not reference the removed status column (no method_missing or pg error)" do
      # This would have raised if where(status: ...) was still in the code.
      export = create_export
      expect { described_class.perform_now(export.id) }.not_to raise_error
    end
  end

  # ── new-style filters hash (from Filters#to_h) ───────────────────────────

  describe "new-style filter hashes" do
    it "review_status filter narrows exported documents" do
      approved = build_doc(ai_status: :completed, review_status: :approved)
      build_doc(ai_status: :completed, review_status: :pending)

      export = create_export("review_status" => "approved")
      described_class.perform_now(export.id)

      expect(export.reload.documents_count).to eq(1)
    end

    it "date_from / date_to filter narrows by document_date" do
      inside  = build_doc(document_date: Date.new(2026, 6, 15))
      outside = build_doc(document_date: Date.new(2026, 5, 15))

      export = create_export("date_from" => "2026-06-01", "date_to" => "2026-06-30")
      described_class.perform_now(export.id)

      expect(export.reload.documents_count).to eq(1)
    end
  end

  # ── legacy stored-filter compatibility ────────────────────────────────────

  describe "legacy stored filters" do
    it "year + month keys expand to month range (pre-refactor export)" do
      inside  = build_doc(document_date: Date.new(2026, 6, 15))
      outside = build_doc(document_date: Date.new(2026, 5, 15))

      export = create_export("year" => "2026", "month" => "6")
      described_class.perform_now(export.id)

      expect(export.reload.documents_count).to eq(1)
    end

    it "scalar type key is wrapped into an array before filtering" do
      # Old export action stored { "type" => "<id>" } (scalar, not array).
      dt = create(:document_type, workspace: workspace, name: "Receipt", category: "accounting")
      matched   = build_doc.tap { |d| d.update_columns(document_type_id: dt.id) }
      unmatched = build_doc

      export = create_export("type" => dt.id.to_s)
      described_class.perform_now(export.id)

      expect(export.reload.documents_count).to eq(1)
    end

    it "category key narrows by DocumentType category" do
      acct_type = create(:document_type, workspace: workspace, name: "Invoice", category: "accounting")
      legal_type = create(:document_type, workspace: workspace, name: "Contract", category: "legal")
      acct_doc  = build_doc.tap { |d| d.update_columns(document_type_id: acct_type.id) }
      legal_doc = build_doc.tap { |d| d.update_columns(document_type_id: legal_type.id) }

      export = create_export("category" => "accounting")
      described_class.perform_now(export.id)

      expect(export.reload.documents_count).to eq(1)
    end
  end

  # ── job lifecycle ─────────────────────────────────────────────────────────

  describe "job lifecycle" do
    it "marks the export as generated on success" do
      export = create_export
      described_class.perform_now(export.id)
      expect(export.reload.status).to eq("generated")
    end

    it "marks the export as failed and re-raises when an error occurs" do
      allow(Exports::ZipGenerator).to receive(:new).and_raise(RuntimeError, "boom")

      export = create_export
      build_doc

      expect { described_class.perform_now(export.id) }.to raise_error(RuntimeError, "boom")
      expect(export.reload.status).to eq("failed")
    end

    it "stores documents_count on the export" do
      3.times { build_doc }
      export = create_export
      described_class.perform_now(export.id)
      expect(export.reload.documents_count).to eq(3)
    end
  end
end
