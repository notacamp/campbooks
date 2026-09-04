# frozen_string_literal: true

require "rails_helper"

# Tests for the amount-aware settlement helpers introduced in the partial-payment
# backbone (allocated_cents). These complement the existing TransactionMatch spec
# which covers the legacy full-settlement behaviour (documents without amount_cents).
#
# All dates are pinned via travel_to to avoid calendar-drift failures.
RSpec.describe Document, type: :model do
  # Shared infrastructure for every example.
  let(:workspace)     { Workspace.create!(name: "Settlement WS") }
  let(:user)          { workspace.users.create!(name: "Ada", email_address: "ada-settle@example.com", password: "password123") }
  let(:stmt_doc) do
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                    review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("d"), filename: "s.csv", content_type: "text/csv")
    doc.save!
    doc
  end
  let(:reconciliation) do
    Reconciliation.create!(workspace: workspace, created_by: user,
                           statement_document: stmt_doc, currency: "EUR")
  end

  def make_doc(amount_cents:)
    doc = workspace.documents.build(document_type: :expense_invoice,
                                    ai_status: :completed, review_status: :approved,
                                    source: :email)
    doc.amount_cents = amount_cents
    doc.currency     = "EUR"
    doc.original_file.attach(io: StringIO.new("f"), filename: "inv.pdf",
                              content_type: "application/pdf")
    doc.save!
    doc
  end

  def make_txn(amount_cents:, booked_on:, position:)
    BankTransaction.create!(reconciliation: reconciliation, workspace: workspace,
                            position: position, booked_on: booked_on,
                            description: "Payment", amount_cents: amount_cents,
                            currency: "EUR", raw_data: {})
  end

  def confirm_match!(bank_tx, document, allocated_cents:)
    TransactionMatch.create!(bank_transaction: bank_tx, document: document,
                             status: :confirmed, matched_by: :manual,
                             match_reasons: {}, allocated_cents: allocated_cents)
  end

  around do |example|
    travel_to(Time.zone.local(2024, 6, 15, 12, 0, 0)) { example.run }
  end

  # ── settled_amount_cents ────────────────────────────────────────────────────

  describe "#settled_amount_cents" do
    it "returns 0 with no confirmed matches" do
      doc = make_doc(amount_cents: 10_000)
      expect(doc.settled_amount_cents).to eq(0)
    end

    it "sums allocated_cents across confirmed matches" do
      doc   = make_doc(amount_cents: 10_000)
      txn1  = make_txn(amount_cents: -6_000, booked_on: Date.current, position: 0)
      txn2  = make_txn(amount_cents: -4_000, booked_on: Date.current, position: 1)
      confirm_match!(txn1, doc, allocated_cents: 6_000)
      confirm_match!(txn2, doc, allocated_cents: 4_000)
      expect(doc.settled_amount_cents).to eq(10_000)
    end

    it "ignores suggested and rejected matches" do
      doc  = make_doc(amount_cents: 10_000)
      txn  = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)
      TransactionMatch.create!(bank_transaction: txn, document: doc,
                               status: :suggested, matched_by: :heuristic,
                               match_reasons: {}, allocated_cents: 10_000)
      expect(doc.settled_amount_cents).to eq(0)
    end
  end

  # ── outstanding_cents ───────────────────────────────────────────────────────

  describe "#outstanding_cents" do
    it "returns nil when the document has no amount" do
      doc = make_doc(amount_cents: nil)
      # amount_cents nil → can't compute outstanding
      # (force nil by overwriting metadata)
      doc.update_columns(metadata: doc.metadata.except("amount_cents"))
      expect(doc.outstanding_cents).to be_nil
    end

    it "equals amount_cents when nothing is settled" do
      doc = make_doc(amount_cents: 10_000)
      expect(doc.outstanding_cents).to eq(10_000)
    end

    it "returns the remainder after partial allocation" do
      doc  = make_doc(amount_cents: 10_000)
      txn  = make_txn(amount_cents: -3_000, booked_on: Date.current, position: 0)
      confirm_match!(txn, doc, allocated_cents: 3_000)
      expect(doc.outstanding_cents).to eq(7_000)
    end

    it "clamps to 0, never goes negative" do
      doc  = make_doc(amount_cents: 10_000)
      txn  = make_txn(amount_cents: -12_000, booked_on: Date.current, position: 0)
      confirm_match!(txn, doc, allocated_cents: 12_000)
      expect(doc.outstanding_cents).to eq(0)
    end
  end

  # ── settlement_state ────────────────────────────────────────────────────────

  describe "#settlement_state" do
    it "is :unsettled with no amount" do
      doc = make_doc(amount_cents: nil)
      doc.update_columns(metadata: doc.metadata.except("amount_cents"))
      expect(doc.settlement_state).to eq(:unsettled)
    end

    it "is :unsettled before any confirmed match" do
      doc = make_doc(amount_cents: 10_000)
      expect(doc.settlement_state).to eq(:unsettled)
    end

    it "is :partial when some but not all is allocated (deposit scenario)" do
      doc  = make_doc(amount_cents: 10_000)
      txn  = make_txn(amount_cents: -3_000, booked_on: Date.current, position: 0)
      confirm_match!(txn, doc, allocated_cents: 3_000)
      expect(doc.settlement_state).to eq(:partial)
      expect(doc.outstanding_cents).to eq(7_000)
    end

    it "is :settled when allocation equals amount (1:1 full match)" do
      doc  = make_doc(amount_cents: 10_000)
      txn  = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)
      confirm_match!(txn, doc, allocated_cents: 10_000)
      expect(doc.settlement_state).to eq(:settled)
      expect(doc.outstanding_cents).to eq(0)
    end

    it "is :settled when two instalments together cover the invoice (within 1 cent)" do
      doc  = make_doc(amount_cents: 10_000)
      txn1 = make_txn(amount_cents: -6_000, booked_on: Date.current, position: 0)
      txn2 = make_txn(amount_cents: -4_000, booked_on: Date.current + 1, position: 1)
      confirm_match!(txn1, doc, allocated_cents: 6_000)
      confirm_match!(txn2, doc, allocated_cents: 4_000)
      expect(doc.settlement_state).to eq(:settled)
    end

    it "accepts a 1-cent rounding tolerance for :settled" do
      doc  = make_doc(amount_cents: 10_000)
      txn  = make_txn(amount_cents: -9_999, booked_on: Date.current, position: 0)
      confirm_match!(txn, doc, allocated_cents: 9_999)
      expect(doc.settlement_state).to eq(:settled)
    end
  end

  # ── recompute_bank_settlement! ──────────────────────────────────────────────

  describe "#recompute_bank_settlement!" do
    context "full 1:1 payment" do
      it "sets settled_at and bank_match source when fully settled" do
        doc  = make_doc(amount_cents: 10_000)
        txn  = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)
        confirm_match!(txn, doc, allocated_cents: 10_000)

        doc.reload
        expect(doc.settled_at.to_date).to eq(Date.current)
        expect(doc.settled_source).to eq("bank_match")
      end
    end

    context "split — 1 payment, 2 invoices" do
      it "settles each invoice independently" do
        doc1 = make_doc(amount_cents: 6_000)
        doc2 = make_doc(amount_cents: 4_000)
        txn  = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)

        confirm_match!(txn, doc1, allocated_cents: 6_000)
        confirm_match!(txn, doc2, allocated_cents: 4_000)

        expect(doc1.reload.settled_source).to eq("bank_match")
        expect(doc2.reload.settled_source).to eq("bank_match")
      end
    end

    context "installments — 2 payments, 1 invoice" do
      it "is :partial after the first instalment" do
        doc  = make_doc(amount_cents: 10_000)
        txn1 = make_txn(amount_cents: -6_000, booked_on: Date.current, position: 0)
        confirm_match!(txn1, doc, allocated_cents: 6_000)

        doc.reload
        expect(doc.settlement_state).to eq(:partial)
        # Bank-match settlement not written yet — invoice still outstanding.
        expect(doc.settled_at).to be_nil
      end

      it "is :settled and writes bank_match settlement once fully paid" do
        doc  = make_doc(amount_cents: 10_000)
        txn1 = make_txn(amount_cents: -6_000, booked_on: Date.current,     position: 0)
        txn2 = make_txn(amount_cents: -4_000, booked_on: Date.current + 1, position: 1)
        confirm_match!(txn1, doc, allocated_cents: 6_000)
        confirm_match!(txn2, doc, allocated_cents: 4_000)

        doc.reload
        expect(doc.settlement_state).to eq(:settled)
        expect(doc.settled_at.to_date).to eq(Date.current + 1)  # latest booked_on
        expect(doc.settled_source).to eq("bank_match")
      end
    end

    context "partial deposit — payment covers only part of the invoice" do
      it "does not set settled_at (invoice still outstanding)" do
        doc  = make_doc(amount_cents: 10_000)
        txn  = make_txn(amount_cents: -3_000, booked_on: Date.current, position: 0)
        confirm_match!(txn, doc, allocated_cents: 3_000)

        doc.reload
        expect(doc.settled_at).to be_nil
        expect(doc.settled_source).to be_nil
        expect(doc.outstanding_cents).to eq(7_000)
      end
    end

    context "unsettling" do
      it "clears bank_match settlement when the only confirmed match is removed" do
        doc   = make_doc(amount_cents: 10_000)
        txn   = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)
        match = confirm_match!(txn, doc, allocated_cents: 10_000)
        expect(doc.reload.settled_source).to eq("bank_match")

        match.destroy!
        expect(doc.reload.settled_at).to be_nil
        expect(doc.reload.settled_source).to be_nil
      end

      it "drops from :settled to :partial when one instalment is rejected" do
        doc   = make_doc(amount_cents: 10_000)
        txn1  = make_txn(amount_cents: -6_000, booked_on: Date.current, position: 0)
        txn2  = make_txn(amount_cents: -4_000, booked_on: Date.current, position: 1)
        _m1   = confirm_match!(txn1, doc, allocated_cents: 6_000)
        m2    = confirm_match!(txn2, doc, allocated_cents: 4_000)
        expect(doc.reload.settlement_state).to eq(:settled)

        m2.update!(status: :rejected)
        doc.reload
        expect(doc.settlement_state).to eq(:partial)
        expect(doc.settled_at).to be_nil
      end

      it "preserves a manual settlement when a bank match is rejected" do
        doc = make_doc(amount_cents: 10_000)
        doc.mark_settled!
        expect(doc.reload).to be_settled_manual

        txn   = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)
        match = TransactionMatch.create!(bank_transaction: txn, document: doc,
                                         status: :suggested, matched_by: :heuristic,
                                         match_reasons: {}, allocated_cents: 10_000)
        match.update!(status: :rejected)

        expect(doc.reload).to be_settled_manual
      end
    end
  end

  # ── TransactionMatch.allocated_cents defaulting ─────────────────────────────

  describe "TransactionMatch allocated_cents default" do
    it "defaults to the document's amount_cents when not supplied" do
      doc   = make_doc(amount_cents: 8_500)
      txn   = make_txn(amount_cents: -8_500, booked_on: Date.current, position: 0)
      match = TransactionMatch.create!(bank_transaction: txn, document: doc,
                                       status: :suggested, matched_by: :heuristic,
                                       match_reasons: {})
      expect(match.allocated_cents).to eq(8_500)
    end

    it "keeps an explicit allocated_cents value when supplied" do
      doc   = make_doc(amount_cents: 10_000)
      txn   = make_txn(amount_cents: -4_000, booked_on: Date.current, position: 0)
      match = TransactionMatch.create!(bank_transaction: txn, document: doc,
                                       status: :suggested, matched_by: :manual,
                                       match_reasons: {}, allocated_cents: 4_000)
      expect(match.allocated_cents).to eq(4_000)
    end

    it "rejects a negative allocated_cents" do
      doc   = make_doc(amount_cents: 10_000)
      txn   = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)
      match = TransactionMatch.new(bank_transaction: txn, document: doc,
                                   status: :suggested, matched_by: :manual,
                                   match_reasons: {}, allocated_cents: -1)
      expect(match).not_to be_valid
      expect(match.errors[:allocated_cents]).to be_present
    end
  end
end
