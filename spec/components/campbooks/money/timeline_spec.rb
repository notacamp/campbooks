# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Money::Timeline, type: :component do
  let(:today) { Date.new(2026, 9, 3) }
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace:) }

  def revenue(**attrs)
    create(:document, :approved, :revenue_invoice, workspace:, currency: "EUR", **attrs)
  end

  def expense(**attrs)
    create(:document, :approved, workspace:, document_type: :expense_invoice, currency: "EUR", **attrs)
  end

  def render_timeline
    ledger = Money::Ledger.for(workspace, user, today:)
    summary = Money::Summary.for(workspace, user, today:, ledger:)
    ApplicationController.render(described_class.new(ledger:, summary:, today:), layout: false)
  end

  it "draws a bar per obligation with a tooltip, linked to its ledger row" do
    doc = revenue(client_name: "Acme", invoice_number: "0234", amount_cents: 222_000, due_date: today + 12)
    html = render_timeline

    expect(html).to include("viewBox")
    expect(html).to include("<title>") # hover tooltip
    expect(html).to include("#ob-doc:#{doc.id}") # bar anchors into the ledger row
    expect(html).to include("Acme")               # a label for the largest bar
  end

  it "colours a late bar destructive, shades the overdue region, and marks today" do
    expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: today - 20)
    html = render_timeline

    expect(html).to include("var(--destructive)")     # the late bar fill
    expect(html).to include("var(--secondary)")       # the shaded overdue region
    expect(html).to include("Today")
    expect(html).to include("20 days late")           # the late label
  end

  it "renders the lane totals from the summary" do
    revenue(client_name: "Acme", invoice_number: "1", amount_cents: 222_000, due_date: today + 12)
    expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: today - 20)
    html = render_timeline

    expect(html).to include("Owed to you")
    expect(html).to include("You owe")
  end

  it "does not emit NaN for a zero-amount obligation" do
    expense(vendor_name: "Zero", amount_cents: 0, due_date: today - 1)
    expect(render_timeline).not_to include("NaN")
  end

  describe "overflow (a deep backlog of old bills)" do
    it "folds obligations older than the window into an 'older' marker, not a clamped bar" do
      old = expense(vendor_name: "Ancient Co", amount_cents: 500_000, due_date: today - 400)
      html = render_timeline

      expect(html).to include("older")                 # the overflow marker label
      expect(html).to include("#money_ledger")         # links to the full ledger
      expect(html).not_to include("#ob-doc:#{old.id}") # never drawn as an on-axis bar
    end

    it "still draws recent overdue bills (inside the window) as individual bars" do
      recent = expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: today - 10)
      expect(render_timeline).to include("#ob-doc:#{recent.id}")
    end

    it "caps labels to at most the two largest bars per lane" do
      6.times do |i|
        revenue(client_name: "Client #{i}", invoice_number: i.to_s, amount_cents: (i + 1) * 10_000, due_date: today + 3 + i)
      end
      html = render_timeline

      # Only bar labels use font-size 11.5; markers/ticks/lane labels use other sizes.
      expect(html.scan('font-size="11.5"').size).to be <= 2
    end

    it "folds far-future due dates into a 'later' marker on the right" do
      future = expense(vendor_name: "Annual Co", amount_cents: 120_000, due_date: today + 75)
      html = render_timeline

      expect(html).to include("later")
      expect(html).not_to include("#ob-doc:#{future.id}")
    end

    it "keeps the axis unchanged when nothing overflows" do
      revenue(client_name: "Acme", invoice_number: "1", amount_cents: 222_000, due_date: today + 12)
      html = render_timeline

      expect(html).not_to include("stroke-dasharray") # no gutter divider
      expect(html).not_to include("#money_ledger")    # no overflow markers
    end
  end
end
