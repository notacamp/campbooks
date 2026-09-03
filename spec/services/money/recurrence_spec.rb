# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::Recurrence do
  let(:today) { Date.new(2026, 9, 3) }
  let(:workspace) { create(:workspace) }

  def expense(vendor:, due:, amount:)
    create(:document, :approved, workspace: workspace, document_type: :expense_invoice,
                                 vendor_name: vendor, currency: "EUR", amount_cents: amount, due_date: due)
  end

  it "detects a monthly subscription and projects the next renewal" do
    expense(vendor: "Cloudhost", due: today - 65, amount: 24_800)
    expense(vendor: "Cloudhost", due: today - 35, amount: 24_800)
    current = expense(vendor: "Cloudhost", due: today - 5, amount: 24_800)

    result = described_class.for(workspace.documents.money_types, today: today)
    info = result[described_class.group_key(current)]

    expect(info).to be_present
    expect(info).to be_recurring
    expect(info.cadence).to eq(:monthly)
    expect(info.next_renewal_on).to eq((today - 5) + 1.month)
  end

  it "detects a yearly subscription" do
    d = expense(vendor: "Seguro", due: today - 730, amount: 41_200)
    expense(vendor: "Seguro", due: today - 365, amount: 41_000)
    expense(vendor: "Seguro", due: today, amount: 41_500)

    info = described_class.for(workspace.documents.money_types, today: today)[described_class.group_key(d)]
    expect(info.cadence).to eq(:yearly)
  end

  it "does not treat a wandering amount as recurring" do
    expense(vendor: "Adhoc", due: today - 60, amount: 10_000)
    expense(vendor: "Adhoc", due: today - 30, amount: 30_000) # +200%, well past tolerance
    d = expense(vendor: "Adhoc", due: today, amount: 10_000)

    expect(described_class.for(workspace.documents.money_types, today: today)[described_class.group_key(d)]).to be_nil
  end

  it "needs at least three occurrences" do
    expense(vendor: "Twice", due: today - 30, amount: 5_000)
    d = expense(vendor: "Twice", due: today, amount: 5_000)

    expect(described_class.for(workspace.documents.money_types, today: today)[described_class.group_key(d)]).to be_nil
  end

  it "keeps the two directions apart" do
    key = described_class.group_key(build(:document, document_type: :revenue_invoice, client_name: "Acme"))
    expect(key).to eq([ "acme", :receivable ])
  end
end
