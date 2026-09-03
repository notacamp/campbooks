# frozen_string_literal: true

require "rails_helper"

RSpec.describe Feed::Sources::LateReceivable do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:noon)      { Time.current.noon }
  subject(:source) { described_class.new(user, now: noon) }

  def invoice(**attrs)
    create(:document, :approved, :revenue_invoice, workspace: workspace, currency: "EUR", **attrs)
  end

  describe "#candidates" do
    it "surfaces a late, unsettled revenue invoice as an attention card" do
      doc = invoice(client_name: "Brightloop", invoice_number: "0231", amount_cents: 120_000, due_date: noon.to_date - 12)
      candidate = source.candidates.find { |c| c[:subject] == doc }

      expect(candidate).to be_present
      expect(candidate[:attention]).to be(true)
      expect(candidate[:score]).to be >= 86
      expect(candidate[:data]["days_late"]).to eq(12)
      expect(candidate[:data]["amount_cents"]).to eq(120_000)
      expect(candidate[:dedupe_key]).to eq("late_receivable:#{doc.id}")
    end

    it "excludes settled, not-yet-due, rejected, and payable documents" do
      invoice(client_name: "Paid", amount_cents: 1_000, due_date: noon.to_date - 5,
              settled_at: Time.current, settled_source: "manual")
      invoice(client_name: "Future", amount_cents: 1_000, due_date: noon.to_date + 5)
      create(:document, :rejected, :revenue_invoice, workspace: workspace, amount_cents: 1_000, due_date: noon.to_date - 5)
      create(:document, :approved, workspace: workspace, document_type: :expense_invoice, amount_cents: 1_000, due_date: noon.to_date - 5)

      expect(source.candidates).to be_empty
    end
  end

  describe "#still_valid?" do
    it "holds while the invoice is late and unsettled, and drops once settled" do
      doc = invoice(amount_cents: 1_000, due_date: noon.to_date - 3)
      expect(source.still_valid?(nil, doc)).to be(true)

      doc.mark_settled!
      expect(source.still_valid?(nil, doc)).to be(false)
    end

    it "drops when the subject is gone" do
      expect(source.still_valid?(nil, nil)).to be(false)
    end
  end
end
