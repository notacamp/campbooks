# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::Summary do
  let(:today) { Date.new(2026, 9, 3) }
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def revenue(**attrs)
    create(:document, :approved, :revenue_invoice, workspace: workspace, currency: "EUR", **attrs)
  end

  def expense(**attrs)
    create(:document, :approved, workspace: workspace, document_type: :expense_invoice, currency: "EUR", **attrs)
  end

  def summary = described_class.for(workspace, user, today: today)

  it "sums each direction and excludes renewals from the open totals" do
    revenue(client_name: "Acme", invoice_number: "1", amount_cents: 222_000, due_date: today + 12)
    revenue(client_name: "Brightloop", invoice_number: "2", amount_cents: 120_000, due_date: today - 12)
    expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: today - 20)
    expense(vendor_name: "Staples", amount_cents: 36_400, due_date: today + 5)
    create(:reminder, workspace: workspace, source: create(:document, :approved, workspace: workspace, document_type: :insurance_policy),
                      reminder_type: :renewal, amount_cents: 41_200, currency: "EUR", due_at: (today + 28).to_time)

    expect(summary.owed_to_you_total.format).to eq("€3,420.00")
    expect(summary.you_owe_total.format).to eq("€612.00") # renewal (€412) is NOT counted
    expect(summary.owe_count).to eq(2)
    expect(summary.renewals.size).to eq(1)
  end

  it "names the largest late item each direction" do
    revenue(client_name: "Brightloop", invoice_number: "2", amount_cents: 120_000, due_date: today - 12)
    expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: today - 20)

    expect(summary.late_receivable.counterpart).to eq("Brightloop")
    expect(summary.late_payable.counterpart).to eq("Cloudhost")
    expect(summary.late_receivable_count).to eq(1)
    expect(summary.late_payable_count).to eq(1)
  end

  it "renders the workspace currency first" do
    expense(vendor_name: "US", amount_cents: 10_000, currency: "USD", due_date: today + 5)
    expense(vendor_name: "EU", amount_cents: 20_000, currency: "EUR", due_date: today + 5)
    expect(summary.you_owe.map { |m| m.currency.iso_code }).to eq(%w[EUR USD])
  end

  describe "nothing_else_this_month?" do
    it "is false while due items remain this month" do
      expense(vendor_name: "Soon", amount_cents: 1_000, due_date: today + 5)
      expect(summary.nothing_else_this_month?).to be(false)
    end

    it "is true when the only open items are overdue or land next month" do
      expense(vendor_name: "Overdue", amount_cents: 1_000, due_date: today - 5)
      expense(vendor_name: "NextMonth", amount_cents: 1_000, due_date: today.end_of_month + 3)
      expect(summary.nothing_else_this_month?).to be(true)
    end
  end
end
