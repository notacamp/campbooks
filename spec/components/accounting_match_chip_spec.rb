# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Accounting::MatchChip, type: :component do
  let(:workspace) { Workspace.create!(name: "Chip WS") }
  let(:user) do
    workspace.users.create!(name: "U", email_address: "u-chip@example.com", password: "password123")
  end
  let(:document) do
    doc = workspace.documents.build(
      document_type: :expense_invoice, ai_status: :completed, review_status: :pending,
      source: :manual_upload, vendor_name: "Vodafone", invoice_number: "FT 129833",
      document_date: Date.new(2026, 6, 4), amount_cents: 4590
    )
    doc.original_file.attach(io: StringIO.new("pdf"), filename: "f.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end
  let(:transaction) do
    rec = Reconciliation.create!(workspace: workspace, statement_document: document, created_by: user)
    rec.bank_transactions.create!(
      workspace: workspace, position: 1, booked_on: Date.new(2026, 6, 5),
      description: "TRF VODAFONE", amount_cents: -4590, currency: "EUR"
    )
  end
  let(:match) do
    transaction.transaction_matches.create!(document: document, status: :suggested,
                                            matched_by: :heuristic, confidence: 0.9)
  end

  def render_chip(**opts)
    ApplicationController.render(described_class.new(match: match, **opts), layout: false)
  end

  it "links to the document in a new tab so the user can open it to confirm the match" do
    html = render_chip

    expect(html).to include(%(href="#{Rails.application.routes.url_helpers.document_path(document)}"))
    expect(html).to include('target="_blank"')
    expect(html).to include('rel="noopener"')
    expect(html).to include("FT 129833")
  end

  it "renders a plain non-link chip when link: false" do
    html = render_chip(link: false)

    expect(html).not_to include("<a ")
    expect(html).to include("Vodafone")
  end
end
