# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::BlankReprocessor do
  let(:workspace) { create(:workspace) }

  # A review-queue document with no extractable fields and an analyzable (PDF) file.
  # Forced to a free-form type with empty metadata so ExtractedFieldSet yields nothing.
  def blank_pdf
    create(:document, :in_review, workspace: workspace, metadata: {})
      .tap { |d| d.update_columns(document_type: Document.document_types[:contract], document_type_id: nil) }
      .reload
  end

  def candidates_for(ws)
    docs = []
    described_class.run(dry_run: true, only_workspace_id: ws.id) { |d, _| docs << d }
    docs
  end

  describe "candidate selection (dry run)" do
    it "selects blank, analyzable, pending documents" do
      doc = blank_pdf
      expect(candidates_for(workspace).map(&:id)).to eq([ doc.id ])
    end

    it "skips documents that already have extracted data" do
      blank_pdf
      create(:document, :in_review, workspace: workspace, vendor_name: "Acme Lda") # expense_invoice w/ a column value

      expect(candidates_for(workspace).count).to eq(1)
    end

    it "skips non-analyzable files (calendar invites, archives, raw emails)" do
      ics = blank_pdf
      ics.original_file.attach(io: StringIO.new("BEGIN:VCALENDAR"), filename: "invite.ics", content_type: "text/calendar")

      expect(candidates_for(workspace)).to be_empty
    end

    it "never touches an approved document (re-analysis would un-approve it)" do
      create(:document, :approved, workspace: workspace, metadata: {})
        .update_columns(document_type: Document.document_types[:contract], document_type_id: nil)

      expect(candidates_for(workspace)).to be_empty
    end

    it "honours LIMIT" do
      3.times { blank_pdf }
      expect(candidates_for_with_limit(workspace, 2).count).to eq(2)
    end
  end

  describe "applying" do
    it "runs the real processor for each candidate when a document provider is configured" do
      doc = blank_pdf
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :documents).and_return(true)
      processor = instance_double(Documents::Processor, call: true)
      expect(Documents::Processor).to receive(:new).with(doc).and_return(processor)

      described_class.run(dry_run: false, only_workspace_id: workspace.id)
    end

    it "reports no_provider (and does not process) when documents aren't configured" do
      blank_pdf
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :documents).and_return(false)
      expect(Documents::Processor).not_to receive(:new)

      totals = described_class.run(dry_run: false, only_workspace_id: workspace.id)
      expect(totals[:no_provider]).to eq(1)
    end
  end

  def candidates_for_with_limit(ws, limit)
    docs = []
    described_class.run(dry_run: true, limit: limit, only_workspace_id: ws.id) { |d, _| docs << d }
    docs
  end
end
