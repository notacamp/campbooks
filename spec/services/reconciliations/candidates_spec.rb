# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::Candidates, type: :service do
  let(:workspace)      { create(:workspace) }
  let(:reconciliation) { create(:reconciliation, :with_period, workspace:, created_by: create(:user, workspace:)) }
  let(:booked_on)      { Date.new(2024, 1, 15) }

  # Helper: build a BankTransaction-like struct for unit tests that don't need DB
  def stub_txn(amount_cents:, counterparty: nil, description: "ACME Corp payment",
               currency: "EUR", booked_on: self.booked_on)
    OpenStruct.new(
      amount_cents:             amount_cents,
      counterparty:             counterparty,
      description:              description,
      currency:                 currency,
      booked_on:                booked_on,
      candidate_document_types: %w[expense_invoice receipt credit_note]
    )
  end

  # Helper: build a Document-like struct for unit tests
  def stub_doc(id:, amount_cents:, vendor_name: nil, client_name: nil,
               currency: "EUR", document_date: booked_on, due_date: nil)
    OpenStruct.new(
      id:            id,
      amount_cents:  amount_cents,
      vendor_name:   vendor_name,
      client_name:   client_name,
      currency:      currency,
      document_date: document_date,
      due_date:      due_date
    )
  end

  # ── score_doc (amount + date + name) ────────────────────────────────────────

  describe "#score_doc" do
    subject(:cands) { described_class.new(bank_transaction: txn, workspace:) }

    let(:txn) { stub_txn(amount_cents: -5000, counterparty: "ACME Corp") }

    it "returns 1.0 for exact amount + same-day + exact name match" do
      doc = stub_doc(id: 1, amount_cents: 5000, vendor_name: "ACME Corp")
      # 0.5 (exact amount) + 0.25 (same day) + 0.25 (name score at 100% similarity) = 1.0
      expect(cands.score_doc(doc)).to be_within(0.01).of(1.0)
    end

    it "returns 0.5 for exact amount only (date outside window, no name)" do
      doc = stub_doc(id: 1, amount_cents: 5000, document_date: booked_on - 200.days)
      expect(cands.score_doc(doc)).to eq(0.5)
    end

    it "returns 0.35 for close amount + no date match + no name" do
      doc = stub_doc(id: 1, amount_cents: 5050, document_date: booked_on - 200.days)
      expect(cands.score_doc(doc)).to eq(0.35)
    end

    it "returns 0.0 for completely different amount" do
      doc = stub_doc(id: 1, amount_cents: 99_000, document_date: booked_on - 200.days)
      expect(cands.score_doc(doc)).to eq(0.0)
    end
  end

  # ── build_reasons ────────────────────────────────────────────────────────────

  describe "#build_reasons" do
    subject(:cands) { described_class.new(bank_transaction: txn, workspace:) }

    let(:txn) { stub_txn(amount_cents: -5000, counterparty: "ACME Corp") }

    it "marks exact amount" do
      doc     = stub_doc(id: 1, amount_cents: 5000)
      reasons = cands.build_reasons(doc, 0.5)
      expect(reasons["amount"]).to eq("exact")
    end

    it "marks close amount" do
      doc     = stub_doc(id: 1, amount_cents: 5050)
      reasons = cands.build_reasons(doc, 0.35)
      expect(reasons["amount"]).to eq("close")
    end

    it "records date_delta_days" do
      doc     = stub_doc(id: 1, amount_cents: 5000, document_date: booked_on - 3.days)
      reasons = cands.build_reasons(doc, 0.5)
      expect(reasons["date_delta_days"]).to eq(3)
    end

    it "records name_similarity when > 0" do
      doc     = stub_doc(id: 2, amount_cents: 5000, vendor_name: "ACME Corp")
      reasons = cands.build_reasons(doc, 0.75)
      expect(reasons["name_similarity"]).to be > 0
    end

    it "records cross_reconciliation_warning when doc id is in set" do
      doc     = stub_doc(id: 42, amount_cents: 5000)
      cands2  = described_class.new(bank_transaction: txn, workspace:, cross_recon_doc_ids: Set.new([ 42 ]))
      reasons = cands2.build_reasons(doc, 0.5)
      expect(reasons["cross_reconciliation_warning"]).to be true
    end
  end

  # ── below_threshold ──────────────────────────────────────────────────────────

  describe "#below_threshold" do
    let(:txn) { create(:bank_transaction, reconciliation:, workspace:, amount_cents: -20_000, booked_on:) }

    it "returns only candidates scored between 0 (exclusive) and SCORE_AUTO_SUGGEST (exclusive)" do
      # doc_high: amount so different it scores 0 → excluded
      # doc_mid: close amount → scores 0.35 → below_threshold (SCORE_AUTO_SUGGEST = 0.85)
      # doc_exact: exact amount + same day + same vendor → scores ≥ 0.85 → excluded from below_threshold
      # doc_mid: close amount (~1.5% off), document_date 60 days before booked_on
      # (within pool's 90-day window, but >= DATE_DECAY_DAYS=30 so date_score = 0)
      # Total score = 0.35 (close amount) → below threshold (0.85)
      doc_mid = create(:document, workspace:,
                       document_type: :expense_invoice,
                       amount_cents: 20_300,   # ~1.5% off → close
                       currency: "EUR",
                       document_date: booked_on - 60.days)  # in pool window, date_score = 0

      doc_exact = create(:document, workspace:,
                         document_type: :expense_invoice,
                         amount_cents: 20_000,
                         currency: "EUR",
                         vendor_name: "PaymentCo",
                         document_date: booked_on)

      txn_with_counterparty = create(:bank_transaction, :with_counterparty,
                                     reconciliation:, workspace:,
                                     amount_cents: -20_000,
                                     booked_on:,
                                     counterparty: "PaymentCo")

      cands = described_class.new(bank_transaction: txn_with_counterparty, workspace:)
      ids   = cands.below_threshold.map { |h| h[:document].id }

      # doc_mid should appear (0 < score < threshold), doc_exact should not (score ≥ threshold)
      expect(ids).to include(doc_mid.id)
      expect(ids).not_to include(doc_exact.id)
    end

    it "excludes candidates with score == 0" do
      # A document whose amount is radically different → scores 0
      create(:document, workspace:,
             document_type: :expense_invoice,
             amount_cents: 1_000_000,
             currency: "EUR",
             document_date: booked_on - 200.days)

      cands = described_class.new(bank_transaction: txn, workspace:)
      expect(cands.below_threshold).to be_empty
    end
  end

  # ── sorting ──────────────────────────────────────────────────────────────────

  describe "#call ordering" do
    let(:txn) { stub_txn(amount_cents: -10_000) }

    it "returns results sorted by score descending" do
      cands  = described_class.new(bank_transaction: txn, workspace:)
      high   = stub_doc(id: 1, amount_cents: 10_000, document_date: booked_on)
      medium = stub_doc(id: 2, amount_cents: 10_100, document_date: booked_on - 200.days)
      low    = stub_doc(id: 3, amount_cents: 99_000, document_date: booked_on - 200.days)

      allow(cands).to receive(:pool).and_return([ medium, low, high ])

      result = cands.call
      expect(result.map { |h| h[:document].id }).to eq([ 1, 2, 3 ])
    end
  end
end
