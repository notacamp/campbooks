# frozen_string_literal: true

require "rails_helper"

RSpec.describe LoanInstalment, type: :model do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:loan) do
    loan = create(:loan, workspace: workspace, created_by: user, term_months: 3)
    Loans::Schedule.build!(loan)
    loan
  end
  let(:reconciliation) do
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                    review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("s"), filename: "s.csv", content_type: "text/csv")
    doc.save!
    Reconciliation.create!(workspace: workspace, created_by: user,
                           statement_document: doc, currency: "EUR", status: :ready)
  end

  describe "after_destroy callback" do
    it "resets the bank transaction to unmatched when the instalment is destroyed" do
      txn = BankTransaction.create!(
        reconciliation: reconciliation, workspace: workspace,
        position: 1, booked_on: Date.current,
        description: "PREST", amount_cents: -40_000,
        currency: "EUR", status: :explained
      )
      inst = loan.instalments.first
      inst.update!(bank_transaction: txn, status: :paid)
      inst.destroy!
      expect(txn.reload.status).to eq("unmatched")
    end
  end

  describe "before_update callback (unlinking)" do
    it "resets the bank transaction when bank_transaction_id is cleared" do
      txn = BankTransaction.create!(
        reconciliation: reconciliation, workspace: workspace,
        position: 2, booked_on: Date.current,
        description: "PREST", amount_cents: -40_000,
        currency: "EUR", status: :explained
      )
      inst = loan.instalments.first
      inst.update!(bank_transaction: txn, status: :paid)
      inst.update!(bank_transaction_id: nil, status: :expected)
      expect(txn.reload.status).to eq("unmatched")
    end
  end
end
