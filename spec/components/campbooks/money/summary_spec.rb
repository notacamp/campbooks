# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Money::Summary, type: :component do
  let(:today) { Date.new(2026, 9, 3) }
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace:) }

  def expense(**attrs)
    create(:document, :approved, workspace:, document_type: :expense_invoice, currency: "EUR", **attrs)
  end

  def render_summary
    ledger = Money::Ledger.for(workspace, user, today:)
    summary = Money::Summary.for(workspace, user, today:, ledger:)
    ApplicationController.render(described_class.new(summary:), layout: false)
  end

  describe "the one that matters" do
    it "names the most pressing late bill and ends the sentence cleanly when nothing is known about it yet" do
      expense(vendor_name: "Newcomer", amount_cents: 120_000, due_date: today - 12)
      html = render_summary

      expect(html).to include("The one that matters is Newcomer")
      expect(html).to include("days late") # the overdue phrase counts from the real current date
      expect(html).not_to match(/—\s*\./) # no dangling dash before the period
    end

    it "appends the why when the counterpart's history explains the bill" do
      3.times do |i|
        expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: today - 60 - (i * 30),
                settled_at: (today - 62 - (i * 30)).to_time, settled_source: "manual")
      end
      expense(vendor_name: "Cloudhost", amount_cents: 74_400, due_date: today - 12)
      html = render_summary

      expect(html).to include("The one that matters is Cloudhost")
      expect(html).to include("3x their usual")
      expect(html).to include("you pay them on time")
    end
  end
end
