# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Paper::RowMenu, type: :component do
  def menu_for(document, folders: [])
    ApplicationController.render(described_class.new(document: document, folders: folders), layout: false)
  end

  def money_invoice(**attrs)
    create(:document, :approved, document_type: :expense_invoice, amount_cents: 5_000,
                                 tax_amount_cents: nil, tax_rate: nil, source: :email, **attrs)
  end

  it "always offers open, download, the exports and reprocess" do
    html = menu_for(money_invoice)
    %w[Open Download Send\ to\ Drive Send\ to\ Notion Reprocess].each do |label|
      expect(html).to include(label.tr("\\", ""))
    end
  end

  it "offers Mark paid for an unsettled money document" do
    html = menu_for(money_invoice)
    expect(html).to include("Mark paid")
    expect(html).not_to include("Mark unpaid")
  end

  it "offers Mark unpaid only for a manually-settled money document" do
    html = menu_for(money_invoice(settled_at: Time.current, settled_source: "manual"))
    expect(html).to include("Mark unpaid")
    expect(html).not_to include("Mark paid")
  end

  it "offers no settle toggle for a non-money document" do
    contract = create(:document, :approved, document_type: :contract, amount_cents: nil,
                                            tax_amount_cents: nil, tax_rate: nil, source: :email)
    html = menu_for(contract)
    expect(html).not_to include("Mark paid")
    expect(html).not_to include("Mark unpaid")
  end

  it "shows Delete only for a manual upload" do
    manual = create(:document, :approved, document_type: :receipt, tax_amount_cents: nil, tax_rate: nil,
                                          source: :manual_upload)
    expect(menu_for(manual)).to include("Delete")
    expect(menu_for(money_invoice)).not_to include(">Delete<")
  end

  it "lists the workspace folders under Move to folder" do
    invoice = money_invoice
    folder = create(:mail_folder, workspace: invoice.workspace, name: "Taxes 2026")
    html = menu_for(invoice, folders: [ folder ])
    expect(html).to include("Move to folder")
    expect(html).to include("Taxes 2026")
  end
end
