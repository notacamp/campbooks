# frozen_string_literal: true

require "rails_helper"

RSpec.describe Reconciliations::Matcher, type: :service do
  subject(:matcher) { described_class.new(reconciliation: reconciliation, workspace: workspace) }

  let(:workspace)       { create(:workspace) }
  let(:reconciliation)  { create(:reconciliation, workspace:, created_by: create(:user, workspace:)) }

  # Suppress live broadcasts in unit tests
  before do
    allow(Turbo::StreamsChannel).to receive(:broadcast_replace_to)
    allow(Turbo::StreamsChannel).to receive(:broadcast_update_to)
    # Stub ApplicationController.render to avoid view context setup
    allow(ApplicationController).to receive(:render).and_return("<tr></tr>")
  end

  # ── amount_score ─────────────────────────────────────────────────────────────

  describe "#amount_score (private)" do
    def txn(cents) = OpenStruct.new(amount_cents: cents, booked_on: Date.today, counterparty: nil, description: "Test", currency: "EUR")
    def doc(cents) = OpenStruct.new(amount_cents: cents, document_date: Date.today, due_date: nil, vendor_name: nil, client_name: nil, currency: "EUR")

    def score(t, d) = matcher.send(:amount_score, t, d)

    it "returns 0.5 for an exact match" do
      expect(score(txn(-5000), doc(5000))).to eq(0.5)
    end

    it "returns 0.5 when both are exact positives" do
      expect(score(txn(5000), doc(5000))).to eq(0.5)
    end

    it "returns 0.35 when within 2%" do
      expect(score(txn(-5000), doc(5050))).to eq(0.35)
    end

    it "returns 0 when outside 2%" do
      expect(score(txn(-5000), doc(6000))).to eq(0)
    end

    it "returns 0 when either value is zero" do
      expect(score(txn(0), doc(5000))).to eq(0)
      expect(score(txn(-5000), doc(0))).to eq(0)
    end
  end

  # ── date_score ────────────────────────────────────────────────────────────────

  describe "#date_score (private)" do
    def txn(date) = OpenStruct.new(amount_cents: -5000, booked_on: date, counterparty: nil, description: "Test", currency: "EUR")
    def doc(doc_date, due: nil) = OpenStruct.new(amount_cents: 5000, document_date: doc_date, due_date: due, vendor_name: nil, client_name: nil, currency: "EUR")

    def score(t, d) = matcher.send(:date_score, t, d)

    it "returns 0.25 for same day" do
      day = Date.today
      expect(score(txn(day), doc(day))).to eq(0.25)
    end

    it "returns > 0 for 1 day apart" do
      day = Date.today
      expect(score(txn(day), doc(day + 1))).to be > 0
    end

    it "returns 0 at 30 days apart" do
      day = Date.today
      expect(score(txn(day), doc(day + 30))).to eq(0)
    end

    it "returns 0 when document has no date" do
      expect(score(txn(Date.today), doc(nil))).to eq(0)
    end
  end

  # ── name_score ────────────────────────────────────────────────────────────────

  describe "#name_score (private)" do
    def txn(desc, counterparty: nil) = OpenStruct.new(amount_cents: -5000, booked_on: Date.today, counterparty: counterparty, description: desc, currency: "EUR")
    def doc(vendor) = OpenStruct.new(amount_cents: 5000, document_date: Date.today, due_date: nil, vendor_name: vendor, client_name: nil, currency: "EUR")

    def score(t, d) = matcher.send(:name_score, t, d)

    it "returns 0.25 for identical names" do
      expect(score(txn("ACME CORP"), doc("Acme Corp"))).to be_within(0.01).of(0.25)
    end

    it "returns 0 when Jaccard similarity is below 0.4" do
      expect(score(txn("ACME CORP"), doc("TOTALLY DIFFERENT VENDOR"))).to eq(0)
    end

    it "returns 0 when either name is blank" do
      expect(score(txn("Acme"), doc(nil))).to eq(0)
    end
  end

  # ── CURRENCY_MAP normalization ───────────────────────────────────────────────

  describe "currency normalization (private)" do
    def normalize(str)
      matcher.send(:normalize_currency, str)
    end

    it "normalizes full names" do
      expect(normalize("Euro")).to eq("EUR")
      expect(normalize("euro")).to eq("EUR")
      expect(normalize("us dollar")).to eq("USD")
    end

    it "normalizes symbols" do
      expect(normalize("€")).to eq("EUR")
      expect(normalize("$")).to eq("USD")
      expect(normalize("£")).to eq("GBP")
    end

    it "passes through ISO codes unchanged" do
      expect(normalize("EUR")).to eq("EUR")
      expect(normalize("USD")).to eq("USD")
    end

    it "upcases unknown codes" do
      expect(normalize("czk")).to eq("CZK")
    end
  end

  # ── Full match cycle ─────────────────────────────────────────────────────────

  describe "#call — heuristic matching" do
    let(:txn) do
      create(:bank_transaction,
             reconciliation:,
             workspace:,
             booked_on:     Date.new(2024, 1, 15),
             description:   "Payment ACME CORP",
             amount_cents:  -5000,
             currency:      "EUR")
    end

    context "with an exact-amount, same-day document" do
      let!(:doc) do
        create(:document,
               workspace:,
               document_type: :expense_invoice,
               amount_cents:  5000,
               currency:      "EUR",
               document_date: Date.new(2024, 1, 15),
               vendor_name:   "Acme Corp")
      end

      before { txn } # force creation before matcher.call

      it "creates a suggested TransactionMatch" do
        expect { matcher.call }
          .to change(TransactionMatch, :count).by(1)
      end

      it "assigns heuristic as matched_by" do
        matcher.call
        match = txn.reload.transaction_matches.first
        expect(match.matched_by).to eq("heuristic")
        expect(match.status).to eq("suggested")
      end

      it "upgrades the transaction status to :suggested" do
        matcher.call
        expect(txn.reload.status).to eq("suggested")
      end
    end

    context "with no viable document" do
      before { txn }

      it "leaves transaction as :unmatched" do
        matcher.call
        expect(txn.reload.status).to eq("unmatched")
      end

      it "creates no TransactionMatches" do
        expect { matcher.call }.not_to change(TransactionMatch, :count)
      end
    end

    context "when a document's currency doesn't match" do
      before do
        create(:document,
               workspace:,
               document_type: :expense_invoice,
               amount_cents:  5000,
               currency:      "USD",
               document_date: Date.new(2024, 1, 15))
        txn
      end

      it "skips the cross-currency document" do
        matcher.call
        expect(txn.reload.status).to eq("unmatched")
      end
    end

    context "high-confidence match (≥ 0.85)" do
      let!(:doc) do
        create(:document,
               workspace:,
               document_type: :expense_invoice,
               amount_cents:  5000,
               currency:      "EUR",
               document_date: Date.new(2024, 1, 15),
               vendor_name:   "Payment ACME Corp")
      end

      before { txn }

      it "sets confidence ≥ 0.85 on the match" do
        matcher.call
        match = txn.reload.transaction_matches.first
        expect(match.confidence).to be >= 0.85
      end
    end
  end

  # ── NIF exception count ──────────────────────────────────────────────────────

  describe "#nif_exception_count (private)" do
    let(:doc) { create(:document, workspace:, document_type: :expense_invoice, buyer_nif: "PT123456789") }
    let!(:txn) do
      t = create(:bank_transaction, reconciliation:, workspace:, status: :matched)
      create(:transaction_match,
             bank_transaction: t,
             document: doc,
             status: :suggested,
             confidence: 0.9)
      t
    end

    context "when workspace has a matching NIF" do
      before { allow(workspace).to receive(:company_nif).and_return("123456789") }

      it "returns 0" do
        expect(matcher.send(:nif_exception_count)).to eq(0)
      end
    end

    context "when workspace NIF mismatches the document" do
      before { allow(workspace).to receive(:company_nif).and_return("PT987654321") }

      it "returns 1" do
        expect(matcher.send(:nif_exception_count)).to eq(1)
      end
    end

    context "when workspace has no NIF set" do
      before { allow(workspace).to receive(:company_nif).and_return(nil) }

      it "returns 0" do
        expect(matcher.send(:nif_exception_count)).to eq(0)
      end
    end
  end
end

# ── ground_ai_matches ──────────────────────────────────────────────────────────

RSpec.describe Reconciliations::Matcher, "#ground_ai_matches (private)" do
  subject(:matcher) { described_class.new(reconciliation: reconciliation, workspace: workspace) }

  let(:workspace)      { create(:workspace) }
  let(:reconciliation) { create(:reconciliation, workspace:, created_by: create(:user, workspace:)) }

  let(:txn) do
    OpenStruct.new(id: "txn-1", amount_cents: -8598, booked_on: Date.new(2024, 6, 15),
                   counterparty: "AMAZON.COM.BE", description: "PAYMENT VIA BANCONTACT", currency: "EUR")
  end

  def doc(id:, cents:, vendor: "Spark Mail Limited", date: Date.new(2024, 6, 29), invoice: nil)
    OpenStruct.new(id: id, amount_cents: cents, vendor_name: vendor, client_name: nil,
                   document_date: date, due_date: nil, invoice_number: invoice, currency: "EUR")
  end

  def ai(id, conf, reason = "model says so")
    { "document_id" => id, "confidence" => conf, "reason" => reason }
  end

  def ground(matches, docs)
    matcher.send(:ground_ai_matches, txn, matches, docs.index_by(&:id))
  end

  it "discards a confident AI match whose amount does not fit (the 2026-07-09 prod incident)" do
    spark_receipt = doc(id: "d-receipt", cents: 6999)
    spark_invoice = doc(id: "d-invoice", cents: 6999, invoice: "1AE12226-0003")

    result = ground([ ai("d-receipt", 0.95), ai("d-invoice", 0.95) ], [ spark_receipt, spark_invoice ])

    expect(result).to be_empty # €69.99 (even twice) is not an €85.98 payment
  end

  it "keeps an exact-amount match at the model's confidence" do
    exact = doc(id: "d-exact", cents: 8598, vendor: "Amazon EU", invoice: "AMZ-1")

    result = ground([ ai("d-exact", 0.93) ], [ exact ])

    expect(result.size).to eq(1)
    expect(result.first.first["confidence"]).to eq(0.93)
  end

  it "caps confidence for close-but-not-exact amounts" do
    close = doc(id: "d-close", cents: 8560, invoice: "X-1") # within 2%

    result = ground([ ai("d-close", 0.97) ], [ close ])

    expect(result.first.first["confidence"]).to eq(described_class::AI_CONFIDENCE_CAP_CLOSE)
  end

  it "keeps a split payment whose documents sum to the transaction, capped per doc" do
    a = doc(id: "d-a", cents: 5000, vendor: "Vendor A", invoice: "A-1")
    b = doc(id: "d-b", cents: 3598, vendor: "Vendor B", invoice: "B-1", date: Date.new(2024, 6, 14))

    result = ground([ ai("d-a", 0.9), ai("d-b", 0.9) ], [ a, b ])

    expect(result.size).to eq(2)
    expect(result.map { |m, _| m["confidence"] }).to all(eq(described_class::AI_CONFIDENCE_CAP_SPLIT))
  end

  it "collapses twin documents (same party, amount, date) preferring the one with an invoice number" do
    twin_receipt = doc(id: "d-r", cents: 8598)
    twin_invoice = doc(id: "d-i", cents: 8598, invoice: "INV-9")

    result = ground([ ai("d-r", 0.95), ai("d-i", 0.9) ], [ twin_receipt, twin_invoice ])

    expect(result.size).to eq(1)
    expect(result.first.last.id).to eq("d-i")
  end

  it "ignores hallucinated document ids" do
    expect(ground([ ai("no-such-doc", 0.99) ], [])).to be_empty
  end
end
