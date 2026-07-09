# frozen_string_literal: true

require "rails_helper"

RSpec.describe BankTransaction, type: :model do
  let(:workspace)       { Workspace.create!(name: "BT WS") }
  let(:user)            { workspace.users.create!(name: "Bob", email_address: "bob-bt@example.com", password: "password123") }
  let(:document) do
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped, review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("d"), filename: "s.csv", content_type: "text/csv")
    doc.save!
    doc
  end
  let(:reconciliation)  { Reconciliation.create!(workspace: workspace, created_by: user, statement_document: document, currency: "EUR") }

  let(:transaction) do
    BankTransaction.new(
      reconciliation: reconciliation,
      workspace:      workspace,
      position:       0,
      booked_on:      Date.new(2024, 1, 15),
      description:    "Electricity bill",
      amount_cents:   -5000,
      currency:       "EUR",
      raw_data:       {}
    )
  end

  describe "associations" do
    it { is_expected.to belong_to(:reconciliation) }
    it { is_expected.to belong_to(:workspace) }
    it { is_expected.to have_many(:transaction_matches).dependent(:destroy) }
  end

  describe "validations" do
    it "validates position uniqueness scoped to reconciliation" do
      transaction.save!
      dup = BankTransaction.new(transaction.attributes.except("id").merge(position: 0))
      expect(dup).not_to be_valid
      expect(dup.errors[:position]).to be_present
    end
  end

  describe "#debit?" do
    it "returns true for negative amount" do
      transaction.amount_cents = -100
      expect(transaction.debit?).to be true
    end

    it "returns false for positive amount" do
      transaction.amount_cents = 100
      expect(transaction.debit?).to be false
    end
  end

  describe "#credit?" do
    it "returns true for positive amount" do
      transaction.amount_cents = 100
      expect(transaction.credit?).to be true
    end

    it "returns false for negative amount" do
      transaction.amount_cents = -100
      expect(transaction.credit?).to be false
    end
  end

  describe "#candidate_document_types" do
    it "returns expense types for debits" do
      transaction.amount_cents = -100
      expect(transaction.candidate_document_types).to include("expense_invoice", "receipt")
    end

    it "returns revenue types for credits" do
      transaction.amount_cents = 100
      expect(transaction.candidate_document_types).to include("revenue_invoice")
    end
  end

  describe "scope :ordered" do
    it "orders by position ascending" do
      transaction.save!
      t2 = BankTransaction.create!(reconciliation: reconciliation, workspace: workspace, position: 1,
                                   booked_on: Date.today, description: "B", amount_cents: -200,
                                   currency: "EUR", raw_data: {})
      expect(BankTransaction.ordered.first).to eq(transaction)
      expect(BankTransaction.ordered.last).to eq(t2)
    end
  end
end
