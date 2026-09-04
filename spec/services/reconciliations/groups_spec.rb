# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::Groups, type: :service do
  # ── Shared setup ─────────────────────────────────────────────────────────────

  let(:workspace) { Workspace.create!(name: "Groups WS") }
  let(:user)      { workspace.users.create!(name: "Bob", email_address: "bob-groups@example.com", password: "password123") }
  let(:stmt_doc) do
    d = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                  review_status: :pending, source: :manual_upload)
    d.original_file.attach(io: StringIO.new("s"), filename: "stmt.csv", content_type: "text/csv")
    d.save!
    d
  end
  let(:reconciliation) do
    Reconciliation.create!(workspace: workspace, created_by: user,
                           statement_document: stmt_doc, currency: "EUR")
  end

  # Helpers so examples stay readable.
  def make_doc(amount_cents:, doc_type: :expense_invoice)
    d = workspace.documents.build(document_type: doc_type,
                                  ai_status: :completed, review_status: :approved,
                                  source: :email)
    d.amount_cents = amount_cents
    d.currency     = "EUR"
    d.original_file.attach(io: StringIO.new("f"), filename: "inv.pdf",
                           content_type: "application/pdf")
    d.save!
    d
  end

  def make_txn(amount_cents:, booked_on:, position:)
    BankTransaction.create!(
      reconciliation: reconciliation, workspace: workspace,
      position: position, booked_on: booked_on,
      description: "Txn #{position}", amount_cents: amount_cents,
      currency: "EUR", raw_data: {}
    )
  end

  def match!(txn, doc, status: :confirmed, allocated_cents: nil)
    TransactionMatch.create!(
      bank_transaction: txn, document: doc,
      status: status, matched_by: :manual,
      match_reasons: {},
      allocated_cents: allocated_cents || doc.amount_cents
    )
  end

  def call_service
    described_class.new(reconciliation).call
  end

  around { |ex| travel_to(Time.zone.local(2024, 6, 15, 12, 0, 0)) { ex.run } }

  # ── :unmatched ───────────────────────────────────────────────────────────────

  describe ":unmatched group" do
    it "is :unmatched with no docs and balanced? false (no invoice total)" do
      _txn = make_txn(amount_cents: -5_000, booked_on: Date.current, position: 0)

      groups = call_service
      expect(groups.size).to eq(1)

      g = groups.first
      expect(g.kind).to eq(:unmatched)
      expect(g.bank_transactions.size).to eq(1)
      expect(g.documents).to be_empty
      expect(g.line_total_cents).to eq(5_000)
      expect(g.invoice_total_cents).to eq(0)
      expect(g.allocated_cents).to eq(0)
      expect(g.outstanding_cents).to eq(0)
      expect(g.balanced?).to be false  # 5000 vs 0
    end

    it "ignores rejected matches when classifying as unmatched" do
      txn = make_txn(amount_cents: -5_000, booked_on: Date.current, position: 0)
      doc = make_doc(amount_cents: 5_000)
      match!(txn, doc, status: :rejected)

      groups = call_service
      expect(groups.size).to eq(1)
      expect(groups.first.kind).to eq(:unmatched)
    end
  end

  # ── :one_to_one ──────────────────────────────────────────────────────────────

  describe ":one_to_one group" do
    it "identifies a simple confirmed full match" do
      txn = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)
      doc = make_doc(amount_cents: 10_000)
      match!(txn, doc)

      groups = call_service
      expect(groups.size).to eq(1)

      g = groups.first
      expect(g.kind).to eq(:one_to_one)
      expect(g.bank_transactions).to contain_exactly(txn)
      expect(g.documents).to contain_exactly(doc)
      expect(g.line_total_cents).to eq(10_000)
      expect(g.invoice_total_cents).to eq(10_000)
      expect(g.allocated_cents).to eq(10_000)
      expect(g.outstanding_cents).to eq(0)
      expect(g.balanced?).to be true
    end

    it "includes a suggested match in the same group as confirmed" do
      txn = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)
      doc = make_doc(amount_cents: 10_000)
      match!(txn, doc, status: :suggested)

      g = call_service.first
      expect(g.kind).to eq(:one_to_one)
      expect(g.documents).to contain_exactly(doc)
    end
  end

  # ── :many_to_one ─────────────────────────────────────────────────────────────

  describe ":many_to_one group (2 payments → 1 invoice, fully settled)" do
    it "classifies as :many_to_one when fully paid across two transactions" do
      doc  = make_doc(amount_cents: 10_000)
      txn1 = make_txn(amount_cents: -6_000, booked_on: Date.current,     position: 0)
      txn2 = make_txn(amount_cents: -4_000, booked_on: Date.current + 1, position: 1)
      match!(txn1, doc, allocated_cents: 6_000)
      match!(txn2, doc, allocated_cents: 4_000)

      groups = call_service
      expect(groups.size).to eq(1)

      g = groups.first
      expect(g.kind).to eq(:many_to_one)
      expect(g.bank_transactions).to match_array([ txn1, txn2 ])
      expect(g.documents).to contain_exactly(doc)
      expect(g.line_total_cents).to eq(10_000)
      expect(g.invoice_total_cents).to eq(10_000)
      expect(g.allocated_cents).to eq(10_000)
      expect(g.outstanding_cents).to eq(0)
      expect(g.balanced?).to be true
    end
  end

  # ── :one_to_many ─────────────────────────────────────────────────────────────

  describe ":one_to_many group (1 payment split across 2 invoices)" do
    it "classifies as :one_to_many when one txn covers multiple docs" do
      txn  = make_txn(amount_cents: -10_000, booked_on: Date.current, position: 0)
      doc1 = make_doc(amount_cents: 6_000)
      doc2 = make_doc(amount_cents: 4_000)
      match!(txn, doc1, allocated_cents: 6_000)
      match!(txn, doc2, allocated_cents: 4_000)

      groups = call_service
      expect(groups.size).to eq(1)

      g = groups.first
      expect(g.kind).to eq(:one_to_many)
      expect(g.bank_transactions).to contain_exactly(txn)
      expect(g.documents).to match_array([ doc1, doc2 ])
      expect(g.line_total_cents).to eq(10_000)
      expect(g.invoice_total_cents).to eq(10_000)
      expect(g.allocated_cents).to eq(10_000)
      expect(g.outstanding_cents).to eq(0)
      expect(g.balanced?).to be true
    end
  end

  # ── :partial ─────────────────────────────────────────────────────────────────

  describe ":partial group (deposit < invoice)" do
    it "classifies as :partial when allocated < invoice_total" do
      txn = make_txn(amount_cents: -3_000, booked_on: Date.current, position: 0)
      doc = make_doc(amount_cents: 10_000)
      match!(txn, doc, allocated_cents: 3_000)

      groups = call_service
      expect(groups.size).to eq(1)

      g = groups.first
      expect(g.kind).to eq(:partial)
      expect(g.allocated_cents).to eq(3_000)
      expect(g.outstanding_cents).to eq(7_000)
      expect(g.balanced?).to be false
    end
  end

  # ── ordering ─────────────────────────────────────────────────────────────────

  describe "group ordering" do
    it "orders groups by the earliest booked_on" do
      doc_a = make_doc(amount_cents: 5_000)
      doc_b = make_doc(amount_cents: 8_000)
      txn_a = make_txn(amount_cents: -5_000, booked_on: Date.current + 5, position: 1)
      txn_b = make_txn(amount_cents: -8_000, booked_on: Date.current,     position: 0)
      match!(txn_a, doc_a)
      match!(txn_b, doc_b)

      groups = call_service
      expect(groups.size).to eq(2)
      # txn_b booked earlier → its group comes first
      expect(groups.first.bank_transactions).to contain_exactly(txn_b)
      expect(groups.last.bank_transactions).to contain_exactly(txn_a)
    end
  end

  # ── multiple independent groups ───────────────────────────────────────────────

  describe "multiple groups in one reconciliation" do
    it "produces separate groups for independent matches plus unmatched" do
      doc1 = make_doc(amount_cents: 5_000)
      doc2 = make_doc(amount_cents: 9_000)
      txn1 = make_txn(amount_cents: -5_000, booked_on: Date.current,     position: 0)
      txn2 = make_txn(amount_cents: -9_000, booked_on: Date.current + 1, position: 1)
      txn3 = make_txn(amount_cents: -2_000, booked_on: Date.current + 2, position: 2)  # unmatched

      match!(txn1, doc1)
      match!(txn2, doc2)

      groups = call_service
      expect(groups.size).to eq(3)

      kinds = groups.map(&:kind)
      expect(kinds).to contain_exactly(:one_to_one, :one_to_one, :unmatched)
    end
  end

  # ── many_to_many ─────────────────────────────────────────────────────────────

  describe ":many_to_many group" do
    it "classifies as :many_to_many when multiple txns match multiple docs" do
      doc1 = make_doc(amount_cents: 7_000)
      doc2 = make_doc(amount_cents: 5_000)
      txn1 = make_txn(amount_cents: -8_000, booked_on: Date.current,     position: 0)
      txn2 = make_txn(amount_cents: -4_000, booked_on: Date.current + 1, position: 1)

      match!(txn1, doc1, allocated_cents: 7_000)
      match!(txn1, doc2, allocated_cents: 1_000)
      match!(txn2, doc2, allocated_cents: 4_000)

      groups = call_service
      expect(groups.size).to eq(1)
      expect(groups.first.kind).to eq(:many_to_many)
      expect(groups.first.bank_transactions).to match_array([ txn1, txn2 ])
      expect(groups.first.documents).to match_array([ doc1, doc2 ])
    end
  end
end
