# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::ReminderDraft do
  let(:today) { Date.new(2026, 9, 3) }

  def obligation(**attrs)
    Money::Obligation.new(
      { counterpart: "Brightloop", amount: Money.new(120_000, "EUR"), due_on: Date.new(2026, 8, 22) }.merge(attrs)
    )
  end

  describe ".chase" do
    it "builds a subject and a four-sentence body with the amount, due date and days late" do
      doc = build(:document, :revenue_invoice, invoice_number: "0231")
      draft = described_class.chase(obligation(document: doc), today: today)

      expect(draft.subject).to eq("Invoice #0231 · payment reminder")
      expect(draft.body).to include("Brightloop")
      expect(draft.body).to include("invoice #0231")
      expect(draft.body).to include("€1,200.00")
      expect(draft.body).to include("Aug 22")
      expect(draft.body).to include("12 days ago")
      expect(draft.body.scan(/[.?]/).size).to be >= 4
    end

    it "falls back to a generic subject when there is no invoice number" do
      draft = described_class.chase(obligation(document: nil), today: today)
      expect(draft.subject).to eq("Payment reminder")
      expect(draft.body).to include("this invoice")
    end
  end

  describe ".cancellation" do
    it "addresses the counterpart and the renewal date" do
      draft = described_class.cancellation(obligation(counterpart: "Seguro", due_on: Date.new(2026, 10, 1)), today: today)
      expect(draft.subject).to eq("Seguro · cancellation request")
      expect(draft.body).to include("Seguro")
      expect(draft.body).to include("Oct 1")
    end
  end
end
