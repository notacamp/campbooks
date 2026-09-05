# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::SkimDocuments, type: :service do
  let(:workspace)      { create(:workspace) }
  let(:period_start)   { Date.new(2024, 1, 1) }
  let(:period_end)     { Date.new(2024, 1, 31) }
  let(:reconciliation) do
    create(:reconciliation, workspace:,
           created_by: create(:user, workspace:),
           period_start:,
           period_end:)
  end
  let(:booked_on) { Date.new(2024, 1, 15) }
  let(:txn) do
    create(:bank_transaction, reconciliation:, workspace:,
           amount_cents: -20_000,
           booked_on:)
  end

  subject(:service) { described_class.new(bank_transaction: txn, reconciliation:, workspace:) }

  # ── window ───────────────────────────────────────────────────────────────────

  describe "#window" do
    it "is period_start - 15 days .. period_end + 15 days" do
      expect(service.window.begin).to eq(period_start - 15.days)
      expect(service.window.end).to   eq(period_end   + 15.days)
    end

    context "when reconciliation has no period" do
      let(:reconciliation) do
        create(:reconciliation, workspace:, created_by: create(:user, workspace:),
               period_start: nil, period_end: nil)
      end

      it "falls back to booked_on ± 15 days" do
        expect(service.window.begin).to eq(booked_on - 15.days)
        expect(service.window.end).to   eq(booked_on + 15.days)
      end
    end
  end

  # ── call ─────────────────────────────────────────────────────────────────────

  describe "#call" do
    let(:in_window_date) { period_start + 5.days }

    it "returns money-type documents within the window" do
      doc = create(:document, workspace:,
                   document_type: :expense_invoice,
                   amount_cents:  20_000,
                   document_date: in_window_date)

      expect(service.call).to include(doc)
    end

    it "excludes documents with no amount_cents" do
      # bank_statement trait sets amount_cents nil
      create(:document, :bank_statement, workspace:, document_date: in_window_date)
      expect(service.call).to be_empty
    end

    it "excludes documents already confirmed in this reconciliation" do
      doc = create(:document, workspace:,
                   document_type: :expense_invoice,
                   amount_cents:  20_000,
                   document_date: in_window_date)
      other_txn = create(:bank_transaction, reconciliation:, workspace:, booked_on:)
      create(:transaction_match, bank_transaction: other_txn, document: doc, status: :confirmed)

      expect(service.call).not_to include(doc)
    end

    it "still shows documents confirmed in a DIFFERENT reconciliation" do
      other_recon = create(:reconciliation, workspace:,
                           created_by: create(:user, workspace:))
      other_txn   = create(:bank_transaction, reconciliation: other_recon, workspace:, booked_on:)
      doc = create(:document, workspace:,
                   document_type: :expense_invoice,
                   amount_cents:  20_000,
                   document_date: in_window_date)
      create(:transaction_match, bank_transaction: other_txn, document: doc, status: :confirmed)

      expect(service.call).to include(doc)
    end

    it "sorts results by closest absolute amount to the bank line" do
      doc_far   = create(:document, workspace:, document_type: :expense_invoice,
                          amount_cents: 50_000, document_date: in_window_date)
      doc_close = create(:document, workspace:, document_type: :expense_invoice,
                          amount_cents: 20_500, document_date: in_window_date)
      doc_exact = create(:document, workspace:, document_type: :expense_invoice,
                          amount_cents: 20_000, document_date: in_window_date)

      result = service.call
      expect(result.first).to eq(doc_exact)
      expect(result.second).to eq(doc_close)
      expect(result.last).to   eq(doc_far)
    end

    it "limits results to MAX_RESULTS" do
      described_class::MAX_RESULTS.succ.times do
        create(:document, workspace:, document_type: :expense_invoice,
               amount_cents: 20_000, document_date: in_window_date)
      end

      expect(service.call.size).to be <= described_class::MAX_RESULTS
    end

    it "excludes documents outside the window entirely" do
      doc = create(:document, workspace:,
                   document_type: :expense_invoice,
                   amount_cents:  20_000,
                   document_date: period_end + 120.days)

      expect(service.call).not_to include(doc)
    end
  end
end
