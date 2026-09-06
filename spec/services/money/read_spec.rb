# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::Read do
  let(:today)     { Date.new(2024, 2, 15) }
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let!(:jan_stmt) do
    create(:reconciliation, :ready, :with_bank, workspace: workspace,
           period_start: Date.new(2024, 1, 1),
           period_end:   Date.new(2024, 1, 31))
  end

  def build_read
    ev  = Money::Evidence.for(workspace)
    led = Money::Ledger.for(workspace, user, today: today, evidence: ev)
    described_class.for(workspace, user, today: today, evidence: ev, ledger: led)
  end

  it "reports any_statements? true when a ready statement exists" do
    expect(build_read.any_statements?).to be true
  end

  it "reports any_statements? false when no ready statement" do
    workspace2 = create(:workspace)
    user2 = create(:user, workspace: workspace2)
    ev = Money::Evidence.for(workspace2)
    led = Money::Ledger.for(workspace2, user2, today: today, evidence: ev)
    r = described_class.for(workspace2, user2, today: today, evidence: ev, ledger: led)
    expect(r.any_statements?).to be false
  end

  it "sets statement to the newest ready statement" do
    expect(build_read.statement).to eq(jan_stmt)
  end

  it "counts bank transaction statuses for the newest statement" do
    create(:bank_transaction, reconciliation: jan_stmt, workspace: workspace, status: :matched)
    create(:bank_transaction, reconciliation: jan_stmt, workspace: workspace, status: :unmatched)
    r = build_read
    expect(r.lines_total).to eq(2)
    expect(r.lines_explained).to eq(1)
  end

  it "computes explained_pct as 0 when total is 0" do
    expect(build_read.explained_pct).to eq(0)
  end

  it "counts needs_invoice_count as unmatched transactions" do
    create(:bank_transaction, reconciliation: jan_stmt, workspace: workspace,
           status: :unmatched, amount_cents: -5000)
    expect(build_read.needs_invoice_count).to eq(1)
  end

  it "counts missing_count from the ledger" do
    create(:document, :approved, workspace: workspace,
           document_type: :expense_invoice, amount_cents: 10_000,
           document_date: Date.new(2024, 1, 10))
    expect(build_read.missing_count).to eq(1)
  end

  it "reports statement_label using evidence label_for" do
    # Single-month Jan statement -> "January"
    expect(build_read.statement_label).to eq("January")
  end
end
