# frozen_string_literal: true

require "rails_helper"

RSpec.describe Feed::Sources::LateReceivable do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:noon)      { Time.zone.parse("2024-02-15 12:00:00") }

  # Ready January statement so that January docs are :missing.
  let!(:jan_stmt) do
    create(:reconciliation, :ready, workspace: workspace,
           period_start: Date.new(2024, 1, 1),
           period_end:   Date.new(2024, 1, 31))
  end

  subject(:source) { described_class.new(user, now: noon) }

  def invoice(**attrs)
    create(:document, :approved, :revenue_invoice, workspace: workspace,
           currency: "EUR", **attrs)
  end

  describe "#candidates" do
    it "surfaces a revenue invoice with :missing evidence as a candidate" do
      doc = invoice(client_name: "Brightloop", invoice_number: "0231",
                    amount_cents: 120_000, document_date: Date.new(2024, 1, 10))
      candidate = source.candidates.find { |c| c[:subject] == doc }

      expect(candidate).to be_present
      expect(candidate[:attention]).to be true
      expect(candidate[:data]["days_since"]).to be_a(Integer)
      expect(candidate[:data]["amount_cents"]).to eq(120_000)
      expect(candidate[:data]["anchor_date"]).to be_present
      expect(candidate[:dedupe_key]).to eq("late_receivable:#{doc.id}")
    end

    it "excludes settled revenue invoices" do
      invoice(client_name: "Paid", amount_cents: 1_000,
              document_date: Date.new(2024, 1, 10),
              settled_at: Time.current, settled_source: "manual")
      expect(source.candidates).to be_empty
    end

    it "excludes an invoice with no statement coverage yet (:unconfirmed)" do
      # Feb doc is not covered by Jan statement
      invoice(client_name: "Future", amount_cents: 1_000,
              document_date: Date.new(2024, 2, 10), invoice_number: "F001")
      expect(source.candidates).to be_empty
    end

    it "excludes review-rejected invoices" do
      create(:document, :rejected, :revenue_invoice, workspace: workspace,
             amount_cents: 1_000, document_date: Date.new(2024, 1, 10))
      expect(source.candidates).to be_empty
    end

    it "excludes payable (expense) invoices" do
      create(:document, :approved, workspace: workspace,
             document_type: :expense_invoice, amount_cents: 1_000,
             document_date: Date.new(2024, 1, 10))
      expect(source.candidates).to be_empty
    end
  end

  describe "#still_valid?" do
    it "holds while the invoice has :missing status" do
      doc = invoice(amount_cents: 1_000, document_date: Date.new(2024, 1, 10),
                    invoice_number: "F002")
      expect(source.still_valid?(nil, doc)).to be true
    end

    it "drops once the invoice is settled" do
      doc = invoice(amount_cents: 1_000, document_date: Date.new(2024, 1, 10),
                    invoice_number: "F003")
      doc.mark_settled!
      expect(source.still_valid?(nil, doc)).to be false
    end

    it "drops for a doc with no evidence coverage" do
      doc = invoice(amount_cents: 1_000, document_date: Date.new(2024, 2, 10),
                    invoice_number: "F004")
      expect(source.still_valid?(nil, doc)).to be false
    end

    it "drops when subject is nil" do
      expect(source.still_valid?(nil, nil)).to be false
    end
  end
end
