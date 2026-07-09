# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::ExportJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace:) }
  let(:reconciliation) do
    doc = workspace.documents.build(
      document_type: :bank_statement,
      ai_status:     :skipped,
      review_status: :pending,
      source:        :manual_upload
    )
    doc.original_file.attach(
      io:           StringIO.new("Date,Desc,Amount\n2024-01-05,Rent,-1000.00\n"),
      filename:     "statement.csv",
      content_type: "text/csv"
    )
    doc.save!
    # The controller sets :export_generating synchronously before enqueuing;
    # the job picks it up already in that state.
    create(:reconciliation,
           workspace:,
           created_by:         user,
           statement_document: doc,
           bank_name:          "Test Bank",
           currency:           "EUR",
           period_start:       Date.new(2024, 1, 1),
           period_end:         Date.new(2024, 1, 31),
           status:             :ready,
           export_status:      :export_generating)
  end

  before do
    allow_any_instance_of(described_class).to receive(:broadcast_export_button!).and_return(nil)
    allow(Notifier).to receive(:reconciliation_export_ready)
  end

  describe "happy path" do
    it "transitions to export_generated and attaches a zip" do
      described_class.new.perform(reconciliation.id)

      reconciliation.reload
      expect(reconciliation.export_status).to eq("export_generated")
      expect(reconciliation.export_zip).to be_attached
      expect(reconciliation.export_zip.filename.to_s).to end_with(".zip")
    end

    it "notifies after success" do
      described_class.new.perform(reconciliation.id)
      expect(Notifier).to have_received(:reconciliation_export_ready).with(reconciliation)
    end

    it "includes the bank name in the filename" do
      described_class.new.perform(reconciliation.id)
      expect(reconciliation.reload.export_zip.filename.to_s).to include("test-bank")
    end
  end

  describe "failure path" do
    before do
      allow_any_instance_of(Reconciliations::ZipBuilder).to receive(:call).and_raise(RuntimeError, "builder error")
    end

    it "transitions to export_failed and re-raises" do
      expect {
        described_class.new.perform(reconciliation.id)
      }.to raise_error(RuntimeError, "builder error")

      expect(reconciliation.reload.export_status).to eq("export_failed")
    end

    it "does not send a notification on failure" do
      expect {
        described_class.new.perform(reconciliation.id)
      }.to raise_error(RuntimeError)

      expect(Notifier).not_to have_received(:reconciliation_export_ready)
    end
  end
end
