# frozen_string_literal: true

require "rails_helper"

RSpec.describe Loans::Spotter, type: :service do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  def make_reconciliation(period_start:, period_end:)
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                    review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("s"), filename: "s.csv", content_type: "text/csv")
    doc.save!
    Reconciliation.create!(workspace: workspace, created_by: user,
                           statement_document: doc, currency: "EUR", status: :ready,
                           period_start: period_start, period_end: period_end)
  end

  # Build N monthly bank transactions (unmatched, debit) 30 days apart.
  def make_monthly_txns(recon, count:, description:, counterparty:, amount_cents:, start_date: Date.new(2023, 7, 5))
    (0...count).map do |i|
      booked = start_date >> i
      BankTransaction.create!(
        reconciliation: recon, workspace: workspace,
        position: BankTransaction.count + 1,
        booked_on: booked, description: description,
        counterparty: counterparty, amount_cents: amount_cents,
        currency: "EUR", status: :unmatched
      )
    end
  end

  let(:recon) { make_reconciliation(period_start: Date.new(2023, 7, 1), period_end: Date.new(2024, 6, 30)) }

  describe "basic detection" do
    it "spots a monthly debit with LOAN_KEYWORDS" do
      make_monthly_txns(recon, count: 6, description: "PRESTACAO EMPRESTIMO",
                               counterparty: "MILLENNIUM BCP", amount_cents: -78_000)

      results = described_class.new(workspace).call
      expect(results).not_to be_empty
      expect(results.first.instalment_cents).to eq(78_000)
    end

    it "spots a BANKISH counterparty even without loan keywords" do
      make_monthly_txns(recon, count: 4, description: "DEBITO DIRETO",
                               counterparty: "NOVO BANCO", amount_cents: -50_000)

      results = described_class.new(workspace).call
      expect(results).not_to be_empty
    end

    it "caps results at 2" do
      make_monthly_txns(recon, count: 4, description: "PRESTACAO A",
                               counterparty: "BANK A", amount_cents: -78_000)
      make_monthly_txns(recon, count: 4, description: "PRESTACAO B",
                               counterparty: "BANK B", amount_cents: -40_000, start_date: Date.new(2023, 7, 10))
      make_monthly_txns(recon, count: 4, description: "PRESTACAO C",
                               counterparty: "BANK C", amount_cents: -30_000, start_date: Date.new(2023, 7, 15))

      results = described_class.new(workspace).call
      expect(results.size).to be <= 2
    end
  end

  describe "cadence check" do
    it "rejects a group whose gaps are outside 25-35 days" do
      # Every 10 days — not monthly
      (0...5).each do |i|
        BankTransaction.create!(
          reconciliation: recon, workspace: workspace,
          position: BankTransaction.count + 1,
          booked_on: Date.new(2023, 7, 1) + (i * 10),
          description: "PREST RAPIDO", counterparty: "BANK X",
          amount_cents: -78_000, currency: "EUR", status: :unmatched
        )
      end
      results = described_class.new(workspace).call
      expect(results).to be_empty
    end
  end

  describe "dismissed suggestions" do
    it "excludes a group whose key is in dismissed_loan_suggestions" do
      make_monthly_txns(recon, count: 4, description: "PRESTACAO EMPRESTIMO",
                               counterparty: "MILLENNIUM BCP", amount_cents: -78_000)

      key = "millennium bcp|78000"
      workspace.settings["dismissed_loan_suggestions"] = [ key ]
      workspace.save!

      results = described_class.new(workspace).call
      expect(results.map(&:key)).not_to include(key)
    end
  end

  describe "already tracked loans" do
    it "excludes groups covered by an active loan" do
      make_monthly_txns(recon, count: 4, description: "PRESTACAO EMPRESTIMO",
                               counterparty: "MILLENNIUM BCP", amount_cents: -78_000)

      create(:loan, :with_schedule, workspace: workspace, created_by: user,
                                    source_counterparty: "MILLENNIUM BCP",
                                    instalment_cents: 78_000,
                                    first_instalment_on: Date.new(2023, 7, 5),
                                    term_months: 60)

      results = described_class.new(workspace).call
      expect(results).to be_empty
    end
  end

  describe "rate-step merging" do
    it "merges two groups of same counterparty with <= 5% amount difference" do
      # First 4 months at 78000, next 4 at 79000 (1.3% step)
      make_monthly_txns(recon, count: 4, description: "PREST EMPRESTIMO",
                               counterparty: "BCP", amount_cents: -78_000)
      make_monthly_txns(recon, count: 4, description: "PREST EMPRESTIMO",
                               counterparty: "BCP", amount_cents: -79_000,
                               start_date: Date.new(2023, 11, 5))

      results = described_class.new(workspace).call
      # Should produce one merged suggestion, not two
      bcp_results = results.select { |r| r.source_counterparty =~ /BCP/i }
      expect(bcp_results.size).to eq(1)
    end
  end
end
