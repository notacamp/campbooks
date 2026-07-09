# frozen_string_literal: true

require "rails_helper"

RSpec.describe TransactionMatch, type: :model do
  let(:workspace)      { Workspace.create!(name: "TM WS") }
  let(:user)           { workspace.users.create!(name: "Carol", email_address: "carol-tm@example.com", password: "password123") }
  let(:statement_doc) do
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped, review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("d"), filename: "s.csv", content_type: "text/csv")
    doc.save!
    doc
  end
  let(:reconciliation) { Reconciliation.create!(workspace: workspace, created_by: user, statement_document: statement_doc, currency: "EUR") }
  let(:bank_tx)        { BankTransaction.create!(reconciliation: reconciliation, workspace: workspace, position: 0, booked_on: Date.today, description: "A", amount_cents: -100, currency: "EUR", raw_data: {}) }
  let(:document) do
    doc = workspace.documents.build(document_type: :expense_invoice, ai_status: :completed, review_status: :approved, source: :email)
    doc.original_file.attach(io: StringIO.new("f"), filename: "inv.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end

  describe "uniqueness" do
    it "validates document_id uniqueness scoped to bank_transaction_id" do
      TransactionMatch.create!(bank_transaction: bank_tx, document: document, status: :suggested, matched_by: :heuristic, match_reasons: {})
      dup = TransactionMatch.new(bank_transaction: bank_tx, document: document, status: :suggested, matched_by: :manual, match_reasons: {})
      expect(dup).not_to be_valid
      expect(dup.errors[:document_id]).to be_present
    end
  end

  describe "confidence validation" do
    it "rejects confidence > 1.0" do
      match = TransactionMatch.new(bank_transaction: bank_tx, document: document, confidence: 1.5, match_reasons: {})
      expect(match).not_to be_valid
    end

    it "accepts nil confidence" do
      match = TransactionMatch.new(bank_transaction: bank_tx, document: document, confidence: nil, status: :suggested, matched_by: :heuristic, match_reasons: {})
      expect(match).to be_valid
    end
  end
end
