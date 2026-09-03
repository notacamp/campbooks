# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Paper::Row, type: :component do
  let(:document) do
    create(:document, :approved, document_type: :expense_invoice, amount_cents: 24_800, currency: "EUR",
                                 tax_amount_cents: nil, tax_rate: nil, due_date: Date.current + 5,
                                 vendor_name: "Cloudhost")
  end

  def render_row(layout)
    ApplicationController.render(described_class.new(document: document, layout: layout), layout: false)
  end

  it "renders the desktop table row: facts, the bold amount, the status chip, the kebab" do
    html = render_row(:table)
    expect(html).to include("<tr")
    expect(html).to include("paper_row_")               # the replaceable dom id
    expect(html).to include("Invoice")                  # the kind fact
    expect(html).to include("€248.00")                  # the amount fact
    expect(html).to include("Unpaid")                   # the derived status chip
    expect(html).to include("Cloudhost")                # the title link
    expect(html).to include("<summary")                 # the RowMenu trigger
  end

  it "renders the mobile card with its own dom id" do
    html = render_row(:card)
    expect(html).to include("paper_card_")
    expect(html).to include("Cloudhost")
    expect(html).to include("Invoice")
    expect(html).to include("€248.00")
    expect(html).to include("Unpaid")
  end
end
