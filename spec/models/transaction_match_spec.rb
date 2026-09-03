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

  describe "settlement sync (Paper's paid status)" do
    def match!(status)
      TransactionMatch.create!(bank_transaction: bank_tx, document: document, status: status,
                               matched_by: :manual, match_reasons: {})
    end

    it "writes settled_at (from the transaction's booked_on) + bank_match source on confirm" do
      match!(:confirmed)
      document.reload
      expect(document.settled_at.to_date).to eq(bank_tx.booked_on)
      expect(document.settled_source).to eq("bank_match")
    end

    it "does not settle a merely suggested match" do
      match!(:suggested)
      expect(document.reload.settled_at).to be_nil
    end

    it "clears a bank-match settlement when the confirmed match is rejected" do
      match = match!(:confirmed)
      expect(document.reload).to be_settled
      match.update!(status: :rejected)
      expect(document.reload.settled_at).to be_nil
      expect(document.settled_source).to be_nil
    end

    it "clears a bank-match settlement when the match is destroyed (reset)" do
      match = match!(:confirmed)
      match.destroy!
      expect(document.reload.settled_at).to be_nil
    end

    it "keeps a settlement while another confirmed match remains" do
      other_tx = BankTransaction.create!(reconciliation: reconciliation, workspace: workspace, position: 1,
                                         booked_on: Date.today - 1, description: "B", amount_cents: -100,
                                         currency: "EUR", raw_data: {})
      first = match!(:confirmed)
      TransactionMatch.create!(bank_transaction: other_tx, document: document, status: :confirmed,
                               matched_by: :manual, match_reasons: {})
      first.update!(status: :rejected)
      expect(document.reload).to be_settled_bank_match
    end

    it "does not clear a manual settlement (that belongs to the user, not the bank)" do
      document.mark_settled!
      expect(document.reload).to be_settled_manual
      match!(:suggested).update!(status: :rejected)
      expect(document.reload).to be_settled_manual
    end
  end

  describe "documents:backfill_settled task" do
    before(:all) do
      Rails.application.load_tasks unless Rake::Task.task_defined?("documents:backfill_settled")
    end

    it "settles documents from existing confirmed matches, idempotently" do
      # Simulate pre-migration data: a confirmed match whose document isn't settled yet.
      TransactionMatch.create!(bank_transaction: bank_tx, document: document, status: :confirmed,
                               matched_by: :manual, match_reasons: {})
      document.update_columns(settled_at: nil, settled_source: nil)

      task = Rake::Task["documents:backfill_settled"]
      task.reenable
      task.invoke

      expect(document.reload.settled_source).to eq("bank_match")
    end
  end
end
