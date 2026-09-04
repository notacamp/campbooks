# frozen_string_literal: true

require "rails_helper"

RSpec.describe Feed::Sources::LatePayable do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:noon)      { Time.current.noon }
  subject(:source) { described_class.new(user, now: noon) }

  def invoice(**attrs)
    create(:document, :approved, workspace: workspace, document_type: :expense_invoice,
           currency: "EUR", **attrs)
  end

  describe "#candidates" do
    it "surfaces a late, unsettled expense invoice as an attention card" do
      doc = invoice(vendor_name: "FastHost", invoice_number: "EX-001", amount_cents: 50_000,
                    due_date: noon.to_date - 10)
      candidate = source.candidates.find { |c| c[:subject] == doc }

      expect(candidate).to be_present
      expect(candidate[:attention]).to be true
      expect(candidate[:score]).to be >= 86
      expect(candidate[:data]["days_late"]).to eq(10)
      expect(candidate[:data]["amount_cents"]).to eq(50_000)
      expect(candidate[:data]["due_date"]).to eq((noon.to_date - 10).iso8601)
      expect(candidate[:dedupe_key]).to eq("late_payable:#{doc.id}")
    end

    it "excludes a settled expense invoice" do
      doc = invoice(amount_cents: 1_000, due_date: noon.to_date - 5, settled_at: Time.current,
                    settled_source: "manual")
      expect(source.candidates.map { |c| c[:subject] }).not_to include(doc)
    end

    it "excludes an expense invoice not yet due" do
      doc = invoice(amount_cents: 1_000, due_date: noon.to_date + 3)
      expect(source.candidates.map { |c| c[:subject] }).not_to include(doc)
    end

    it "excludes a review-rejected invoice" do
      doc = create(:document, :rejected, workspace: workspace, document_type: :expense_invoice,
                   amount_cents: 1_000, due_date: noon.to_date - 5)
      expect(source.candidates.map { |c| c[:subject] }).not_to include(doc)
    end

    it "excludes revenue invoices (wrong direction)" do
      doc = create(:document, :approved, :revenue_invoice, workspace: workspace,
                   amount_cents: 1_000, due_date: noon.to_date - 5)
      expect(source.candidates.map { |c| c[:subject] }).not_to include(doc)
    end

    it "scores higher the later the invoice is" do
      mild  = invoice(amount_cents: 1_000, due_date: noon.to_date - 4)
      older = invoice(amount_cents: 1_000, due_date: noon.to_date - 30)

      mild_score  = source.candidates.find { |c| c[:subject] == mild }[:score]
      older_score = source.candidates.find { |c| c[:subject] == older }[:score]
      expect(older_score).to be >= mild_score
    end
  end

  describe "#still_valid?" do
    it "holds while the expense invoice is late and unsettled" do
      doc = invoice(amount_cents: 1_000, due_date: noon.to_date - 5)
      expect(source.still_valid?(nil, doc)).to be true
    end

    it "drops once the invoice is settled" do
      doc = invoice(amount_cents: 1_000, due_date: noon.to_date - 5)
      doc.mark_settled!
      expect(source.still_valid?(nil, doc)).to be false
    end

    it "drops for a revenue invoice (wrong direction)" do
      doc = create(:document, :approved, :revenue_invoice, workspace: workspace,
                   amount_cents: 1_000, due_date: noon.to_date - 5)
      expect(source.still_valid?(nil, doc)).to be false
    end

    it "drops for a future due date" do
      doc = invoice(amount_cents: 1_000, due_date: noon.to_date + 2)
      expect(source.still_valid?(nil, doc)).to be false
    end

    it "drops when subject is nil" do
      expect(source.still_valid?(nil, nil)).to be false
    end
  end
end
