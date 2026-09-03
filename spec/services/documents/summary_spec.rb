# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::Summary do
  let(:today) { Date.new(2026, 9, 3) }
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def invoice(**attrs)
    create(:document, :approved, workspace: workspace, document_type: :expense_invoice,
                                 currency: "EUR", **attrs)
  end

  it "counts open obligations and totals them per currency" do
    invoice(amount_cents: 24_800, due_date: today - 20, vendor_name: "Cloudhost")
    invoice(amount_cents: 36_400, due_date: today + 5, vendor_name: "Staples")
    summary = described_class.for(workspace, user, today: today)

    expect(summary.unpaid_count).to eq(2)
    expect(summary.totals_by_currency).to eq("EUR" => 61_200)
    expect(summary.sentence).to include("2 invoices are unpaid, €612.00 in total.")
  end

  it "names the worst overdue counterpart" do
    invoice(amount_cents: 24_800, due_date: today - 20, vendor_name: "Cloudhost")
    invoice(amount_cents: 10_000, due_date: today - 3, vendor_name: "Staples")
    summary = described_class.for(workspace, user, today: today)

    expect(summary.late_count).to eq(2)
    expect(summary.worst_late[:document].entity_display_name).to eq("Cloudhost")
    expect(summary.sentence).to include("Cloudhost is 20 days overdue.")
  end

  it "excludes settled and needs-review documents from the obligations" do
    invoice(amount_cents: 9_600, due_date: today - 10, settled_at: Time.current, settled_source: "manual")
    create(:document, :in_review, workspace: workspace, document_type: :expense_invoice,
                                  amount_cents: 5_000, due_date: today - 4)
    summary = described_class.for(workspace, user, today: today)

    expect(summary.unpaid_count).to eq(0)
    expect(summary.needs_review_count).to eq(1)
    expect(summary.sentence).to include("One document needs a second look.")
  end

  it "keeps totals separate per currency" do
    invoice(amount_cents: 10_000, due_date: today + 5)
    invoice(amount_cents: 20_000, due_date: today + 5, currency: "USD")
    summary = described_class.for(workspace, user, today: today)
    expect(summary.totals_by_currency).to eq("EUR" => 10_000, "USD" => 20_000)
  end

  describe "no facts" do
    it "falls back to nil (the connect prompt) when nothing is open and AI is off" do
      allow(Ai::ProviderSetup).to receive(:configured?).and_return(false)
      summary = described_class.for(workspace, user, today: today)
      expect(summary.any?).to be(false)
      expect(summary.ai_configured?).to be(false)
      expect(summary.sentence).to be_nil
    end

    it "says nothing needs you when AI is on but nothing is open" do
      allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
      summary = described_class.for(workspace, user, today: today)
      expect(summary.sentence).to eq("Nothing on paper needs you right now.")
    end
  end
end
