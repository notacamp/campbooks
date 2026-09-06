# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::ReminderDraft do
  def obligation(**attrs)
    Money::Obligation.new(
      { counterpart: "Brightloop", amount: Money.new(120_000, "EUR"), anchor_on: Date.new(2026, 8, 22) }.merge(attrs)
    )
  end

  describe ".chase" do
    it "builds a subject and body with the counterpart and amount" do
      doc = build(:document, :revenue_invoice, invoice_number: "0231")
      draft = described_class.chase(obligation(document: doc))

      expect(draft.subject).to eq("Invoice #0231 · payment reminder")
      expect(draft.body).to include("Brightloop")
      expect(draft.body).to include("invoice #0231")
      expect(draft.body).to include("€1,200.00")
      expect(draft.body).to include("isn't showing on our side yet")
    end

    it "falls back to a generic subject when there is no invoice number" do
      draft = described_class.chase(obligation(document: nil))
      expect(draft.subject).to eq("Payment reminder")
      expect(draft.body).to include("this invoice")
    end

    it "does not mention due dates or days late" do
      doc = build(:document, :revenue_invoice, invoice_number: "0231")
      draft = described_class.chase(obligation(document: doc))
      expect(draft.body).not_to match(/days? (ago|late)/i)
      expect(draft.body).not_to match(/due on/i)
    end
  end
end
