# frozen_string_literal: true

require "rails_helper"

RSpec.describe Loans::Matcher, type: :service do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:reconciliation) do
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                    review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("s"), filename: "s.csv", content_type: "text/csv")
    doc.save!
    Reconciliation.create!(workspace: workspace, created_by: user,
                           statement_document: doc, currency: "EUR",
                           status: :ready, period_start: Date.new(2024, 1, 1),
                           period_end: Date.new(2024, 1, 31))
  end

  def make_txn(description:, counterparty:, amount_cents:, booked_on:, status: :unmatched)
    BankTransaction.create!(
      reconciliation: reconciliation, workspace: workspace,
      position: BankTransaction.count + 1,
      booked_on: booked_on, description: description,
      counterparty: counterparty, amount_cents: amount_cents,
      currency: "EUR", status: status
    )
  end

  def make_loan(lender:, source_counterparty:, instalment_cents:, first_instalment_on:, term_months: 60)
    loan = create(:loan, workspace: workspace, created_by: user,
                         lender: lender, source_counterparty: source_counterparty,
                         instalment_cents: instalment_cents,
                         first_instalment_on: first_instalment_on,
                         term_months: term_months)
    Loans::Schedule.build!(loan)
    loan
  end

  before { allow_any_instance_of(described_class).to receive(:broadcast_row).and_return(nil) }

  describe "keyword matching" do
    it "links a line matching LOAN_KEYWORDS in description" do
      loan = make_loan(lender: "Bank X", source_counterparty: "BANK X",
                       instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PRESTACAO EMPRESTIMO", counterparty: "BANK X",
                     amount_cents: -78_000, booked_on: Date.new(2024, 1, 5))

      described_class.new(reconciliation).call

      expect(txn.reload.status).to eq("explained")
      expect(loan.instalments.find_by(number: 1).status).to eq("paid")
    end

    it "links a line matching LOAN_KEYWORDS in counterparty" do
      loan = make_loan(lender: "BCP", source_counterparty: "BCP",
                       instalment_cents: 40_000, first_instalment_on: Date.new(2024, 1, 10))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP BANCO",
                     amount_cents: -40_000, booked_on: Date.new(2024, 1, 10))

      described_class.new(reconciliation).call
      expect(txn.reload.status).to eq("explained")
    end
  end

  describe "lender-token matching" do
    it "links a line whose counterparty tokens overlap the lender" do
      loan = make_loan(lender: "Millennium BCP", source_counterparty: "MILLENNIUM BCP",
                       instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "DEBITO DIRETO", counterparty: "MILLENNIUM",
                     amount_cents: -78_000, booked_on: Date.new(2024, 1, 5))

      described_class.new(reconciliation).call
      expect(txn.reload.status).to eq("explained")
    end
  end

  describe "amount tolerance" do
    it "links a line within 3% of instalment_cents" do
      loan = make_loan(lender: "BCP", source_counterparty: "BCP",
                       instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP",
                     amount_cents: -77_000, booked_on: Date.new(2024, 1, 5)) # 1.3% off

      described_class.new(reconciliation).call
      expect(txn.reload.status).to eq("explained")
    end

    it "does not link a line outside 3% tolerance" do
      make_loan(lender: "BCP", source_counterparty: "BCP",
                instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP",
                     amount_cents: -60_000, booked_on: Date.new(2024, 1, 5)) # 23% off

      described_class.new(reconciliation).call
      expect(txn.reload.status).to eq("unmatched")
    end
  end

  describe "+/-12 day window and catch-up" do
    it "links a line booked 10 days after the expected_on" do
      loan = make_loan(lender: "BCP", source_counterparty: "BCP",
                       instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP",
                     amount_cents: -78_000, booked_on: Date.new(2024, 1, 15)) # 10 days late

      described_class.new(reconciliation).call
      expect(txn.reload.status).to eq("explained")
    end
  end

  describe "rate reset" do
    it "updates instalment_cents when paid amount differs by >= 0.5%" do
      loan = make_loan(lender: "BCP", source_counterparty: "BCP",
                       instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP",
                     amount_cents: -79_000, booked_on: Date.new(2024, 1, 5)) # 1.28% > 0.5%

      described_class.new(reconciliation).call
      expect(loan.reload.instalment_cents).to eq(79_000)
    end

    it "sets previous_amount_cents on the paid instalment" do
      loan = make_loan(lender: "BCP", source_counterparty: "BCP",
                       instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP",
                     amount_cents: -79_000, booked_on: Date.new(2024, 1, 5))

      described_class.new(reconciliation).call
      inst = loan.instalments.find_by!(number: 1)
      expect(inst.previous_amount_cents).to eq(78_000)
    end
  end

  describe "idempotency" do
    it "does not re-link an already explained line" do
      loan = make_loan(lender: "BCP", source_counterparty: "BCP",
                       instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP",
                     amount_cents: -78_000, booked_on: Date.new(2024, 1, 5), status: :explained)
      inst = loan.instalments.find_by!(number: 1)
      inst.update!(bank_transaction: txn, status: :paid)

      expect {
        described_class.new(reconciliation).call
      }.not_to change { inst.reload.updated_at }
    end
  end

  describe "never touches credits or non-debit lines" do
    it "skips credit transactions" do
      make_loan(lender: "BCP", source_counterparty: "BCP",
                instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP",
                     amount_cents: 78_000, booked_on: Date.new(2024, 1, 5)) # positive = credit

      described_class.new(reconciliation).call
      expect(txn.reload.status).to eq("unmatched")
    end
  end

  describe "never touches matched/suggested lines" do
    it "skips already matched lines" do
      make_loan(lender: "BCP", source_counterparty: "BCP",
                instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP",
                     amount_cents: -78_000, booked_on: Date.new(2024, 1, 5), status: :matched)

      described_class.new(reconciliation).call
      expect(txn.reload.status).to eq("matched")
    end
  end

  describe "excluded with reason loan" do
    it "links an excluded-as-loan line (converts it to explained)" do
      loan = make_loan(lender: "BCP", source_counterparty: "BCP",
                       instalment_cents: 78_000, first_instalment_on: Date.new(2024, 1, 5))
      txn = make_txn(description: "PREST 1/60", counterparty: "BCP",
                     amount_cents: -78_000, booked_on: Date.new(2024, 1, 5), status: :excluded)
      txn.update_columns(exclusion_reason: "loan")

      described_class.new(reconciliation).call
      expect(txn.reload.status).to eq("explained")
      expect(loan.instalments.find_by!(number: 1).status).to eq("paid")
    end
  end
end
