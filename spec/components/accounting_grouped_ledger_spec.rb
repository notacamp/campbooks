# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Accounting::ReconciliationGroup, type: :component do
  # ── Shared setup ────────────────────────────────────────────────────────────
  let(:workspace) { Workspace.create!(name: "Group Ledger WS") }
  let(:user) do
    workspace.users.create!(name: "U", email_address: "u-group@example.com",
                            password: "password123")
  end
  let(:statement_doc) do
    doc = workspace.documents.build(
      document_type: :bank_statement, ai_status: :skipped,
      review_status: :pending, source: :manual_upload
    )
    doc.original_file.attach(io: StringIO.new("csv"), filename: "stmt.csv",
                             content_type: "text/csv")
    doc.save!
    doc
  end
  let(:reconciliation) do
    Reconciliation.create!(workspace: workspace, statement_document: statement_doc,
                           created_by: user, status: :ready)
  end

  # ── Helpers ─────────────────────────────────────────────────────────────────
  def make_txn(amount_cents:, status: :matched, description: "Test txn",
               counterparty: "Vendor", booked_on: Date.new(2024, 1, 10))
    reconciliation.bank_transactions.create!(
      workspace: workspace, position: SecureRandom.random_number(10_000),
      booked_on: booked_on, description: description,
      counterparty: counterparty, amount_cents: amount_cents, currency: "EUR",
      status: status
    )
  end

  def make_doc(amount_cents:, vendor_name: "Vendor", invoice_number: "INV-001",
               document_type: :expense_invoice)
    doc = workspace.documents.build(
      document_type: document_type, ai_status: :completed, review_status: :pending,
      source: :manual_upload
    )
    doc.original_file.attach(io: StringIO.new("pdf"), filename: "inv.pdf",
                             content_type: "application/pdf")
    doc.metadata = { "vendor_name" => vendor_name, "invoice_number" => invoice_number,
                     "amount_cents" => amount_cents.to_s }
    doc.save!
    doc
  end

  def make_match(txn, doc, status: :confirmed, confidence: 0.95,
                 allocated_cents: nil)
    txn.transaction_matches.create!(
      document: doc, status: status, matched_by: :heuristic,
      confidence: confidence, allocated_cents: allocated_cents || doc.amount_cents
    )
  end

  def make_group(txns:, docs:, kind:, allocated_cents: nil)
    invoice_total = docs.sum { |d| d.amount_cents.to_i }
    alloc         = allocated_cents || invoice_total
    outstanding   = [ invoice_total - alloc, 0 ].max
    line_total    = txns.sum { |t| t.amount_cents.abs }
    Reconciliations::Groups::Group.new(
      bank_transactions: txns, documents: docs,
      line_total_cents: line_total, invoice_total_cents: invoice_total,
      allocated_cents: alloc, outstanding_cents: outstanding,
      balanced: (line_total - invoice_total).abs <= 1, kind: kind
    )
  end

  def render_component(group, company_nif: nil)
    ApplicationController.render(
      described_class.new(group: group, company_nif: company_nif),
      layout: false
    )
  end

  # ── Tests ────────────────────────────────────────────────────────────────────

  it "renders a 1:1 matched group as a flat row with a check glyph" do
    txn = make_txn(amount_cents: -4590)
    doc = make_doc(amount_cents: 4590, vendor_name: "Vodafone Portugal",
                   invoice_number: "FT2024/00045")
    make_match(txn, doc)
    txn.reload
    group = make_group(txns: [ txn ], docs: [ doc ], kind: :one_to_one)

    html = render_component(group)

    expect(html).to include("Vodafone Portugal")
    expect(html).to include("FT2024/00045")
    # check glyph: no "½" or "!" for a clean 1:1 matched
    expect(html).not_to include("½")
    expect(html).not_to include(">!</")
    # plain "−€45.90" amount
    expect(html).to include("45.90")
  end

  it "renders a many → 1 group with an inset container and balance footer" do
    t1 = make_txn(amount_cents: -60_000, description: "Install 1/2", booked_on: Date.new(2024, 1, 8))
    t2 = make_txn(amount_cents: -60_000, description: "Install 2/2", booked_on: Date.new(2024, 1, 22))
    doc = make_doc(amount_cents: 120_000, vendor_name: "Insight IT",
                   invoice_number: "FT2024/00300")
    make_match(t1, doc, allocated_cents: 60_000)
    make_match(t2, doc, allocated_cents: 60_000)
    t1.reload; t2.reload
    group = make_group(txns: [ t1, t2 ], docs: [ doc ], kind: :many_to_one)

    html = render_component(group)

    expect(html).to include("Insight IT")
    expect(html).to include("Install 1/2")
    expect(html).to include("Install 2/2")
    # balance footer
    expect(html).to include("✓")
  end

  it "renders a partial group with an amber ½ node and outstanding footer" do
    txn = make_txn(amount_cents: -60_000, description: "Sinal de obra")
    doc = make_doc(amount_cents: 150_000, vendor_name: "Studio Reno",
                   invoice_number: "FT2024/00120")
    make_match(txn, doc, allocated_cents: 60_000)
    txn.reload
    group = make_group(txns: [ txn ], docs: [ doc ], kind: :partial, allocated_cents: 60_000)

    html = render_component(group)

    expect(html).to include("½")
    expect(html).to include("Studio Reno")
    expect(html).to include("outstanding")
  end

  it "renders an unmatched group with an ember ! chip" do
    txn = make_txn(amount_cents: -20_000, status: :unmatched,
                   description: "Pagamento fornecedor", counterparty: "Distribuidora Norte")
    group = make_group(txns: [ txn ], docs: [], kind: :unmatched)

    html = render_component(group)

    expect(html).to include("!")
    expect(html).to include("No invoice")
    expect(html).not_to include("FT")
  end

  it "renders a :requested group with a muted waiting label (no Resolve chip)" do
    txn = make_txn(amount_cents: -20_000, status: :requested,
                   description: "Pagamento fornecedor", counterparty: "Distribuidora Norte")
    group = make_group(txns: [ txn ], docs: [], kind: :unmatched)

    html = render_component(group)

    # Shows "Invoice requested" label text (muted, no ember chip)
    expect(html).to include("Invoice requested")
    # Must NOT show the ember "Resolve" / "No invoice" chip for unmatched
    expect(html).not_to include("No invoice")
    # The "!" punctuation used in the unmatched chip must not appear
    expect(html).not_to include(">!</")
  end

  it "renders an excluded group as muted with set-aside label" do
    txn = make_txn(amount_cents: -350, status: :excluded,
                   description: "Comissão conta", counterparty: "BCP")
    txn.update_columns(exclusion_reason: "bank_fee")
    group = make_group(txns: [ txn ], docs: [], kind: :unmatched)

    html = render_component(group)

    expect(html).to include("Set aside")
    expect(html).to include("Bank fee")
  end

  it "shows NIF badge when company_nif is provided and doc has no NIF" do
    txn = make_txn(amount_cents: -8432, status: :suggested,
                   description: "Material escritório", counterparty: "Staples")
    doc = make_doc(amount_cents: 8432, vendor_name: "Staples Portugal",
                   invoice_number: "FT2024/00221")
    make_match(txn, doc, status: :suggested, confidence: 0.78)
    txn.reload
    group = make_group(txns: [ txn ], docs: [ doc ], kind: :one_to_one)

    html = render_component(group, company_nif: "PT500000001")

    expect(html).to include("NIF")
    expect(html).to include("~")
    expect(html).to include("78%")
  end
end

RSpec.describe Campbooks::Accounting::GroupedLedger, type: :component do
  it "renders an empty state when no groups are given" do
    reconciliation = Reconciliation.new.tap { |r| r.id = SecureRandom.uuid }
    html = ApplicationController.render(
      described_class.new(groups: [], reconciliation: reconciliation),
      layout: false
    )
    expect(html).to include("No transactions")
  end
end
