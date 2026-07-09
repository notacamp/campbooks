# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliation, type: :model do
  let(:workspace) { Workspace.create!(name: "Acme WS") }
  let(:user)      { workspace.users.create!(name: "Alice", email_address: "alice-rec@example.com", password: "password123") }
  let(:document) do
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped, review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("data"), filename: "stmt.csv", content_type: "text/csv")
    doc.save!
    doc
  end

  subject(:reconciliation) do
    Reconciliation.new(
      workspace:          workspace,
      created_by:         user,
      statement_document: document,
      currency:           "EUR"
    )
  end

  describe "associations" do
    it { is_expected.to belong_to(:workspace) }
    it { is_expected.to belong_to(:created_by).class_name("User") }
    it { is_expected.to belong_to(:statement_document).class_name("Document") }
    it { is_expected.to have_many(:bank_transactions).dependent(:destroy) }
  end

  describe "enums" do
    it "defines status enum" do
      expect(described_class.statuses.keys).to include("pending", "parsing", "matching", "ready", "failed")
    end

    it "defines export_status enum with prefix" do
      expect(reconciliation).to respond_to(:export_none?)
      expect(reconciliation).to respond_to(:export_generating?)
      expect(reconciliation).to respond_to(:export_generated?)
      expect(reconciliation).to respond_to(:export_failed?)
    end
  end

  describe "#progress_label" do
    before { reconciliation.save! }

    it "returns '0/0' when there are no transactions" do
      expect(reconciliation.progress_label).to eq("0/0")
    end

    it "counts only resolved transactions" do
      reconciliation.bank_transactions.create!(workspace: workspace, position: 0, booked_on: Date.today,
                                               description: "A", amount_cents: -100, currency: "EUR",
                                               raw_data: {}, status: :unmatched)
      reconciliation.bank_transactions.create!(workspace: workspace, position: 1, booked_on: Date.today,
                                               description: "B", amount_cents: -200, currency: "EUR",
                                               raw_data: {}, status: :matched)
      expect(reconciliation.progress_label).to eq("1/2")
    end
  end

  describe "#period_label" do
    it "returns nil when no period set" do
      expect(reconciliation.period_label).to be_nil
    end

    it "formats period range" do
      reconciliation.period_start = Date.new(2024, 1, 1)
      reconciliation.period_end   = Date.new(2024, 1, 31)
      label = reconciliation.period_label
      expect(label).to include("2024")
    end
  end

  describe "scope :recent" do
    it "orders by created_at desc" do
      reconciliation.save!
      older = Reconciliation.create!(workspace: workspace, created_by: user, statement_document: document, currency: "EUR")
      older.update_columns(created_at: 1.day.ago)
      expect(Reconciliation.recent.first).to eq(reconciliation)
    end
  end
end
