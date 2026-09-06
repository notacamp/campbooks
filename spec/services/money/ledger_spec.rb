# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::Ledger do
  let(:today)     { Date.new(2024, 2, 15) }
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  # A ready January statement so docs in Jan are :missing.
  let!(:jan_stmt) do
    create(:reconciliation, :ready, workspace: workspace,
           period_start: Date.new(2024, 1, 1),
           period_end:   Date.new(2024, 1, 31))
  end

  def evidence
    Money::Evidence.for(workspace)
  end

  def ledger
    described_class.for(workspace, user, today: today, evidence: evidence)
  end

  def expense(**attrs)
    create(:document, :approved, workspace: workspace,
           document_type: :expense_invoice, currency: "EUR",
           amount_cents: 10_000,
           **attrs)
  end

  def revenue(**attrs)
    create(:document, :approved, workspace: workspace,
           document_type: :revenue_invoice, currency: "EUR",
           amount_cents: 10_000,
           **attrs)
  end

  describe "document-only obligations" do
    it "builds obligations from expense and revenue invoices" do
      expense(vendor_name: "Vodafone", document_date: Date.new(2024, 1, 10))
      revenue(client_name: "Acme",    document_date: Date.new(2024, 1, 10), invoice_number: "F01")

      expect(ledger.obligations.map(&:direction).sort).to eq(%i[payable receivable].sort)
    end

    it "excludes documents with no amount_cents or direction" do
      create(:document, :approved, workspace: workspace,
             document_type: :expense_invoice, amount_cents: nil, tax_amount_cents: nil)
      expect(ledger.obligations).to be_empty
    end

    it "excludes review_rejected documents" do
      doc = expense(vendor_name: "Skipped", document_date: Date.new(2024, 1, 10))
      doc.update!(review_status: :rejected)
      expect(ledger.obligations).to be_empty
    end
  end

  describe "status from evidence" do
    it "assigns :missing to a doc whose anchor is covered by the Jan statement" do
      d = expense(vendor_name: "Missing", document_date: Date.new(2024, 1, 10))
      ob = ledger.obligations.find { |o| o.counterpart == "Missing" }
      expect(ob.status).to eq(:missing)
    end

    it "assigns :unconfirmed to a doc with no statement coverage yet" do
      d = expense(vendor_name: "NoCoverage", document_date: Date.new(2024, 2, 10))
      ob = ledger.obligations.find { |o| o.counterpart == "NoCoverage" }
      expect(ob.status).to eq(:unconfirmed)
    end

    it "assigns :settled to a settled doc" do
      d = expense(vendor_name: "Paid", document_date: Date.new(2024, 1, 10),
                  settled_at: (today - 5).to_time, settled_source: "manual")
      ob = ledger.obligations.find { |o| o.counterpart == "Paid" }
      expect(ob.status).to eq(:settled)
    end
  end

  describe "actions per direction and status" do
    it "gives missing payable paid_elsewhere and mark_paid" do
      expense(vendor_name: "Payable", document_date: Date.new(2024, 1, 10))
      ob = ledger.missing.find { |o| o.counterpart == "Payable" }
      expect(ob.actions).to eq(%i[paid_elsewhere mark_paid])
    end

    it "gives missing receivable send_reminder and mark_paid" do
      revenue(client_name: "Receivable", document_date: Date.new(2024, 1, 10), invoice_number: "F02")
      ob = ledger.missing.find { |o| o.counterpart == "Receivable" }
      expect(ob.actions).to eq(%i[send_reminder mark_paid])
    end

    it "gives settled obligations no actions" do
      expense(vendor_name: "Paid", document_date: Date.new(2024, 1, 10),
              settled_at: (today - 5).to_time, settled_source: "manual")
      ob = ledger.settled.find { |o| o.counterpart == "Paid" }
      expect(ob.actions).to be_empty
    end

    it "gives unconfirmed obligations no actions" do
      expense(vendor_name: "NoCoverage", document_date: Date.new(2024, 2, 10))
      ob = ledger.unconfirmed.find { |o| o.counterpart == "NoCoverage" }
      expect(ob.actions).to be_empty
    end
  end

  describe "settled lookback (45 days)" do
    it "includes a doc settled within 45 days" do
      expense(vendor_name: "Recent", document_date: Date.new(2024, 1, 10),
              settled_at: (today - 44).to_time, settled_source: "manual")
      expect(ledger.settled.map(&:counterpart)).to include("Recent")
    end

    it "drops a doc settled more than 45 days ago" do
      expense(vendor_name: "Ancient", document_date: Date.new(2024, 1, 10),
              settled_at: (today - 46).to_time, settled_source: "manual")
      expect(ledger.settled.map(&:counterpart)).not_to include("Ancient")
    end
  end

  describe "#find" do
    it "finds an obligation by doc id" do
      d = expense(vendor_name: "Findable", document_date: Date.new(2024, 1, 10))
      expect(ledger.find("doc:#{d.id}").counterpart).to eq("Findable")
    end

    it "returns nil for an unknown id" do
      expect(ledger.find("doc:nonexistent")).to be_nil
    end
  end

  describe "#missing_cents_by_currency" do
    it "sums missing payable amounts by currency" do
      expense(vendor_name: "EuroBill", document_date: Date.new(2024, 1, 10),
              amount_cents: 20_000, currency: "EUR")
      expense(vendor_name: "EuroBill2", document_date: Date.new(2024, 1, 10),
              amount_cents: 30_000, currency: "EUR")
      result = ledger.missing_cents_by_currency
      expect(result["EUR"]).to eq(50_000)
    end

    it "excludes receivables from the currency totals" do
      revenue(client_name: "Client", document_date: Date.new(2024, 1, 10),
              invoice_number: "F03", amount_cents: 50_000, currency: "EUR")
      expect(ledger.missing_cents_by_currency).to be_empty
    end
  end

  describe "#sections" do
    it "includes missing and settled sections, excluding empty ones" do
      expense(vendor_name: "Missing", document_date: Date.new(2024, 1, 10))
      expense(vendor_name: "Paid", document_date: Date.new(2024, 1, 10),
              settled_at: (today - 5).to_time, settled_source: "manual")

      keys = ledger.sections.map(&:first)
      expect(keys).to include(:missing, :settled)
    end

    it "omits sections with no obligations" do
      expense(vendor_name: "Missing", document_date: Date.new(2024, 1, 10))
      keys = ledger.sections.map(&:first)
      expect(keys).not_to include(:settled)
    end
  end

  describe "permissions" do
    it "excludes documents from another workspace" do
      other_ws = create(:workspace)
      create(:document, :approved, workspace: other_ws,
             document_type: :expense_invoice, amount_cents: 9_999,
             document_date: Date.new(2024, 1, 10))
      expect(ledger.obligations).to be_empty
    end
  end
end
