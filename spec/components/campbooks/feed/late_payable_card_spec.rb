# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Feed::LatePayableCard, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:document) do
    create(:document, :approved, workspace: workspace,
           vendor_name: "FastHost", invoice_number: "EX-042",
           amount_cents: 89_900, currency: "EUR",
           document_date: Date.new(2024, 1, 10))
  end

  def make_item(data = {})
    FeedItem.create!(
      user: user, workspace: workspace, kind: "late_payable", subject: document,
      dedupe_key: "late_payable:#{document.id}", sort_at: Time.current, attention: true,
      data: {
        "anchor_date" => Date.new(2024, 1, 10).iso8601,
        "days_since" => 36, "statement_label" => "January",
        "amount_cents" => 89_900, "currency" => "EUR"
      }.merge(data)
    )
  end

  def render_card(item = make_item)
    ApplicationController.render(described_class.new(item: item, subject: document), layout: false)
  end

  it "names the counterpart and the invoice number in the headline" do
    html = render_card
    expect(html).to include("FastHost")
    expect(html).to include("invoice #EX-042")
  end

  it "shows the statement eyebrow" do
    html = render_card
    expect(html).to include("January")
  end

  it "shows the amount" do
    html = render_card
    expect(html).to include("€899.00")
  end

  it "shows Mark paid, Open in Money and Later" do
    html = render_card
    expect(html).to include("Mark paid")
    expect(html).to include("Open in Money")
    expect(html).to include("Later")
  end

  it "does not contain forbidden language" do
    html = render_card
    expect(html).not_to include("You owe")
    expect(html).not_to match(/\d+ days? late/i)
    expect(html).not_to include("Owed")
  end

  it "falls back to 'an invoice' when there is no invoice number" do
    document.update!(invoice_number: nil)
    expect(render_card).to include("an invoice")
  end

  it "shows generic eyebrow when statement_label is absent" do
    item = make_item("statement_label" => nil)
    html = render_card(item)
    expect(html).to include("Not on a statement")
  end

  it "gracefully falls back to old days_late data without crashing" do
    old_item = FeedItem.create!(
      user: user, workspace: workspace, kind: "late_payable", subject: document,
      dedupe_key: "late_payable_old:#{document.id}", sort_at: Time.current, attention: true,
      data: { "days_late" => 10, "amount_cents" => 89_900, "currency" => "EUR" }
    )
    expect { render_card(old_item) }.not_to raise_error
  end
end
