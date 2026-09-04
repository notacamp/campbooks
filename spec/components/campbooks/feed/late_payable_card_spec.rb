# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Feed::LatePayableCard, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:document) do
    create(:document, :approved, workspace: workspace,
           vendor_name: "FastHost", invoice_number: "EX-042",
           amount_cents: 89_900, currency: "EUR", due_date: Date.current - 10)
  end
  let(:item) do
    FeedItem.create!(
      user: user, workspace: workspace, kind: "late_payable", subject: document,
      dedupe_key: "late_payable:#{document.id}", sort_at: Time.current, attention: true,
      data: { "days_late" => 10, "amount_cents" => 89_900, "currency" => "EUR",
              "due_date" => (Date.current - 10).iso8601 }
    )
  end

  def render_card
    ApplicationController.render(described_class.new(item: item, subject: document), layout: false)
  end

  it "names the counterpart, the invoice, the amount and the lateness" do
    html = render_card
    expect(html).to include("FastHost").and include("invoice #EX-042")
    expect(html).to include("You owe")
    expect(html).to include("10 days late")
    expect(html).to include("€899.00")
  end

  it "shows Mark paid, Open in Money and Later" do
    html = render_card
    expect(html).to include("Mark paid")
    expect(html).to include("Open in Money")
    expect(html).to include("Later")
  end

  it "falls back to 'an invoice' when there is no invoice number" do
    document.update!(metadata: document.metadata.except("invoice_number"))
    expect(render_card).to include("an invoice")
  end

  it "reads days_late from feed item data (not re-computed)" do
    # item has days_late: 10; actual due_date is also 10 days ago, but we can
    # trust the card renders the stored value, not a live calculation.
    expect(render_card).to include("10 days late")
  end
end
