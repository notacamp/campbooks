# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::Facts do
  let(:today) { Date.new(2026, 9, 3) }

  def facts_for(*traits, **attrs)
    described_class.for(create(:document, :approved, *traits, tax_amount_cents: nil, tax_rate: nil, **attrs), today: today)
  end

  it "reads a money invoice: kind, bold amount, the due date" do
    facts = facts_for(document_type: :expense_invoice, amount_cents: 24_800, currency: "EUR",
                      due_date: Date.new(2026, 8, 14))
    expect(facts.to_s).to eq("Invoice · €248.00 · due Aug 14")
    amount = facts.segments.find(&:emphasis)
    expect(amount.text).to eq("€248.00")
  end

  it "reads a receipt with an amount and no due date" do
    facts = facts_for(:receipt, amount_cents: 1_299, currency: "EUR", due_date: nil)
    expect(facts.to_s).to eq("Receipt · €12.99")
  end

  it "reads a contract term and end date (other-year date shows the year)" do
    facts = facts_for(document_type: :contract, amount_cents: nil,
                      period_start: Date.new(2026, 7, 1), period_end: Date.new(2027, 7, 1))
    expect(facts.to_s).to eq("Contract · 12 months · ends Jul 2027")
  end

  it "reads a policy renewal date" do
    facts = facts_for(document_type: :insurance_policy, amount_cents: nil,
                      period_end: Date.new(2026, 10, 1))
    expect(facts.to_s).to eq("Policy · renews Oct 1")
  end

  it "reads a bank statement's transaction and match counts" do
    doc = create(:document, :approved, :bank_statement)
    reconciliation = create(:reconciliation, statement_document: doc, workspace: doc.workspace)
    create_list(:bank_transaction, 3, reconciliation: reconciliation)
    create(:bank_transaction, :matched, reconciliation: reconciliation)
    facts = described_class.for(doc, today: today)
    expect(facts.to_s).to eq("Statement · 4 transactions · 1 matched")
  end

  it "never raises on empty metadata — kind only" do
    doc = create(:document, :approved, document_type: :other)
    doc.update_columns(metadata: {})
    expect { described_class.for(doc.reload, today: today).to_s }.not_to raise_error
    expect(described_class.for(doc, today: today).to_s).to eq("Document")
  end
end
