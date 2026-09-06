# frozen_string_literal: true

require "rails_helper"

RSpec.describe Loans::Backfill, type: :service do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  def make_reconciliation(period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                    review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("s"), filename: "s.csv", content_type: "text/csv")
    doc.save!
    Reconciliation.create!(workspace: workspace, created_by: user,
                           statement_document: doc, currency: "EUR",
                           status: :ready,
                           period_start: period_start, period_end: period_end)
  end

  before { allow_any_instance_of(Loans::Matcher).to receive(:broadcast_row).and_return(nil) }

  it "runs Matcher over the workspace ready reconciliations" do
    recon = make_reconciliation
    loan  = create(:loan, workspace: workspace, created_by: user,
                          lender: "BCP", source_counterparty: "BCP",
                          instalment_cents: 78_000,
                          first_instalment_on: Date.new(2024, 1, 5),
                          term_months: 3)
    Loans::Schedule.build!(loan)

    BankTransaction.create!(
      reconciliation: recon, workspace: workspace,
      position: 1, booked_on: Date.new(2024, 1, 5),
      description: "PREST 1/3", counterparty: "BCP",
      amount_cents: -78_000, currency: "EUR", status: :unmatched
    )

    described_class.call(loan)

    inst = loan.instalments.find_by!(number: 1)
    expect(inst.status).to eq("paid")
  end

  it "also calls Status.refresh!" do
    make_reconciliation
    loan = create(:loan, workspace: workspace, created_by: user,
                         instalment_cents: 78_000,
                         first_instalment_on: Date.new(2024, 1, 5), term_months: 3)
    Loans::Schedule.build!(loan)

    expect(Loans::Status).to receive(:refresh!).with(loan).at_least(:once).and_call_original
    described_class.call(loan)
  end
end
