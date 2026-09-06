# frozen_string_literal: true

require "rails_helper"

RSpec.describe Feed::Sources::LatePayable do
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
    create(:document, :approved, workspace: workspace,
           document_type: :expense_invoice, currency: "EUR", **attrs)
  end

  describe "#candidates" do
    it "surfaces an expense invoice with :missing evidence as a candidate" do
      doc = invoice(vendor_name: "FastHost", invoice_number: "EX-001",
                    amount_cents: 50_000, document_date: Date.new(2024, 1, 10))
      candidate = source.candidates.find { |c| c[:subject] == doc }

      expect(candidate).to be_present
      expect(candidate[:attention]).to be true
      expect(candidate[:data]["days_since"]).to be_a(Integer)
      expect(candidate[:data]["amount_cents"]).to eq(50_000)
      expect(candidate[:data]["anchor_date"]).to be_present
      expect(candidate[:dedupe_key]).to eq("late_payable:#{doc.id}")
    end

    it "excludes a settled expense invoice" do
      doc = invoice(amount_cents: 1_000, document_date: Date.new(2024, 1, 10),
                    settled_at: Time.current, settled_source: "manual")
      expect(source.candidates.map { |c| c[:subject] }).not_to include(doc)
    end

    it "excludes an invoice with no statement coverage yet (:unconfirmed)" do
      # Feb doc is not covered by Jan statement
      doc = invoice(amount_cents: 1_000, document_date: Date.new(2024, 2, 10))
      expect(source.candidates.map { |c| c[:subject] }).not_to include(doc)
    end

    it "excludes a review-rejected invoice" do
      doc = create(:document, :rejected, workspace: workspace,
                   document_type: :expense_invoice, amount_cents: 1_000,
                   document_date: Date.new(2024, 1, 10))
      expect(source.candidates.map { |c| c[:subject] }).not_to include(doc)
    end

    it "excludes revenue invoices (wrong direction)" do
      doc = create(:document, :approved, :revenue_invoice, workspace: workspace,
                   amount_cents: 1_000, document_date: Date.new(2024, 1, 10))
      expect(source.candidates.map { |c| c[:subject] }).not_to include(doc)
    end

    it "includes statement_label in data when available" do
      doc = invoice(amount_cents: 1_000, document_date: Date.new(2024, 1, 10))
      candidate = source.candidates.find { |c| c[:subject] == doc }
      expect(candidate[:data]["statement_label"]).to be_present
    end
  end

  describe "#still_valid?" do
    it "holds while the invoice has :missing status" do
      doc = invoice(amount_cents: 1_000, document_date: Date.new(2024, 1, 10))
      expect(source.still_valid?(nil, doc)).to be true
    end

    it "drops once the invoice is settled" do
      doc = invoice(amount_cents: 1_000, document_date: Date.new(2024, 1, 10))
      doc.mark_settled!
      expect(source.still_valid?(nil, doc)).to be false
    end

    it "drops for a doc with no evidence coverage" do
      doc = invoice(amount_cents: 1_000, document_date: Date.new(2024, 2, 10))
      expect(source.still_valid?(nil, doc)).to be false
    end

    it "drops when subject is nil" do
      expect(source.still_valid?(nil, nil)).to be false
    end
  end
end
