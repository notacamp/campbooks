# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Feed::LateReceivableCard, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:document) do
    create(:document, :approved, :revenue_invoice, workspace: workspace,
                                 client_name: "Brightloop", invoice_number: "0231",
                                 amount_cents: 120_000, currency: "EUR", due_date: Date.current - 12)
  end
  let(:item) do
    FeedItem.create!(
      user: user, workspace: workspace, kind: "late_receivable", subject: document,
      dedupe_key: "late_receivable:#{document.id}", sort_at: Time.current, attention: true,
      data: { "days_late" => 12, "amount_cents" => 120_000, "currency" => "EUR" }
    )
  end

  def render_card
    ApplicationController.render(described_class.new(item: item, subject: document), layout: false)
  end

  it "names the counterpart, the invoice, the amount and the lateness" do
    html = render_card
    expect(html).to include("Brightloop").and include("paid invoice #0231")
    expect(html).to include("Owed to you")
    expect(html).to include("12 days late")
    expect(html).to include("€1,200.00")
  end

  it "carries Scout's note that a reminder is drafted" do
    expect(render_card).to include("I drafted a reminder.")
  end

  it "offers Send reminder (chase), Mark paid and Later" do
    html = render_card
    expect(html).to include("Send reminder")
    expect(html).to include("Mark paid")
    expect(html).to include("Later")
    # Send reminder opens the compose Dock via the Money chase path.
    expect(html).to include("/money/obligations/doc:#{document.id}/chase")
  end

  it "falls back to 'an invoice' when there is no invoice number" do
    document.update!(metadata: document.metadata.except("invoice_number"))
    expect(render_card).to include("paid an invoice")
  end
end
