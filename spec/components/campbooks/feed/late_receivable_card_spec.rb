# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Feed::LateReceivableCard, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:document) do
    create(:document, :approved, :revenue_invoice, workspace: workspace,
           client_name: "Brightloop", invoice_number: "0231",
           amount_cents: 120_000, currency: "EUR",
           document_date: Date.new(2024, 1, 10))
  end

  def make_item(data = {})
    FeedItem.create!(
      user: user, workspace: workspace, kind: "late_receivable", subject: document,
      dedupe_key: "late_receivable:#{document.id}", sort_at: Time.current, attention: true,
      data: {
        "anchor_date" => Date.new(2024, 1, 10).iso8601,
        "days_since" => 36, "statement_label" => "January",
        "amount_cents" => 120_000, "currency" => "EUR"
      }.merge(data)
    )
  end

  def render_card(item = make_item)
    ApplicationController.render(described_class.new(item: item, subject: document), layout: false)
  end

  it "names the counterpart and invoice number in the headline" do
    html = render_card
    expect(html).to include("Brightloop")
    expect(html).to include("0231")
  end

  it "shows the statement eyebrow" do
    html = render_card
    expect(html).to include("January")
  end

  it "shows the amount" do
    html = render_card
    expect(html).to include("€1,200.00")
  end

  it "offers Send reminder, Mark paid and Later" do
    html = render_card
    expect(html).to include("Send reminder")
    expect(html).to include("Mark paid")
    expect(html).to include("Later")
    expect(html).to include("/money/obligations/doc:#{document.id}/chase")
  end

  it "does not contain forbidden language" do
    html = render_card
    expect(html).not_to include("Owed to you")
    expect(html).not_to match(/\d+ days? late/i)
    expect(html).not_to include("You owe")
  end

  it "falls back to 'An invoice' when there is no invoice number" do
    document.update!(invoice_number: nil)
    expect(render_card).to include("An invoice")
  end

  it "shows generic eyebrow when statement_label is absent" do
    item = make_item("statement_label" => nil)
    html = render_card(item)
    expect(html).to include("Not on a statement")
  end

  it "gracefully renders old days_late data without crashing" do
    old_item = FeedItem.create!(
      user: user, workspace: workspace, kind: "late_receivable", subject: document,
      dedupe_key: "late_receivable_old:#{document.id}", sort_at: Time.current, attention: true,
      data: { "days_late" => 12, "amount_cents" => 120_000, "currency" => "EUR" }
    )
    expect { render_card(old_item) }.not_to raise_error
  end
end
