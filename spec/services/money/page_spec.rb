# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::Page do
  let(:today)     { Date.new(2024, 2, 15) }
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  let!(:jan_stmt) do
    create(:reconciliation, :ready, :with_bank, workspace: workspace,
           period_start: Date.new(2024, 1, 1),
           period_end:   Date.new(2024, 1, 31))
  end

  def build_page(statement_id: nil)
    described_class.for(workspace, user, today: today, statement_id: statement_id)
  end

  describe "#selected_statement" do
    it "defaults to the newest ready statement when no statement_id given" do
      expect(build_page.selected_statement).to eq(jan_stmt)
    end

    it "uses the given statement_id when it belongs to the workspace" do
      expect(build_page(statement_id: jan_stmt.id).selected_statement).to eq(jan_stmt)
    end

    it "falls back to newest when statement_id belongs to another workspace" do
      other_ws   = create(:workspace)
      other_stmt = create(:reconciliation, :ready, workspace: other_ws,
                          period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
      expect(build_page(statement_id: other_stmt.id).selected_statement).to eq(jan_stmt)
    end

    it "returns nil when there are no ready statements" do
      ws2   = create(:workspace)
      user2 = create(:user, workspace: ws2)
      expect(described_class.for(ws2, user2, today: today).selected_statement).to be_nil
    end
  end

  describe "#needs_you" do
    it "builds :no_invoice items for unmatched debit transactions" do
      create(:bank_transaction, reconciliation: jan_stmt, workspace: workspace,
             status: :unmatched, amount_cents: -5000, counterparty: "EDP")
      items = build_page.needs_you
      expect(items.map(&:kind)).to include(:no_invoice)
      expect(items.find { |i| i.kind == :no_invoice }.title).to eq("No invoice for this payment")
    end

    it "does not include unmatched credits as :no_invoice" do
      create(:bank_transaction, reconciliation: jan_stmt, workspace: workspace,
             status: :unmatched, amount_cents: 10_000)
      expect(build_page.needs_you.map(&:kind)).not_to include(:no_invoice)
    end

    it "caps at NEEDS_YOU_CAP items" do
      9.times do |i|
        create(:bank_transaction, reconciliation: jan_stmt, workspace: workspace,
               status: :unmatched, amount_cents: -5000,
               counterparty: "Vendor#{i}", position: i + 1)
      end
      expect(build_page.needs_you.size).to eq(Money::Page::NEEDS_YOU_CAP)
    end

    it "reports needs_you_overflow count" do
      9.times do |i|
        create(:bank_transaction, reconciliation: jan_stmt, workspace: workspace,
               status: :unmatched, amount_cents: -5000,
               counterparty: "Vendor#{i}", position: i + 1)
      end
      expect(build_page.needs_you_overflow).to eq(1)
    end

    it "returns empty needs_you when no statement" do
      ws2   = create(:workspace)
      user2 = create(:user, workspace: ws2)
      expect(described_class.for(ws2, user2, today: today).needs_you).to be_empty
    end
  end

  describe "#missing" do
    it "delegates to ledger.missing" do
      create(:document, :approved, workspace: workspace,
             document_type: :expense_invoice, amount_cents: 10_000,
             document_date: Date.new(2024, 1, 10))
      expect(build_page.missing.size).to eq(1)
    end
  end

  describe "#statements" do
    it "returns every ready statement on the month timeline" do
      7.times do |i|
        create(:reconciliation, :ready, workspace: workspace,
               period_start: Date.new(2023, i + 1, 1),
               period_end:   Date.new(2023, i + 1, 28))
      end
      expect(build_page.statements.size).to eq(8)
    end
  end

  describe "#months" do
    it "runs from the month to reconcile back to the first statement, newest first" do
      create(:reconciliation, :ready, workspace: workspace,
             period_start: Date.new(2023, 11, 1), period_end: Date.new(2023, 11, 30))
      expect(build_page.months.map(&:starts_on))
        .to eq([ Date.new(2024, 1, 1), Date.new(2023, 12, 1), Date.new(2023, 11, 1) ])
    end
  end

  describe "the month to reconcile in needs_you" do
    it "leads with the month whose statement isn't in yet" do
      page  = described_class.for(workspace, user, today: Date.new(2024, 3, 15))
      first = page.needs_you.first
      expect(first.kind).to eq(:add_statement)
      expect(first.payload[:label]).to eq("February")
    end

    it "says nothing about it while the month to reconcile has its statement" do
      expect(build_page.needs_you.map(&:kind)).not_to include(:add_statement)
    end

    it "leads with the statements Scout is holding when there are unreconciled ones" do
      create(:document, :bank_statement, :approved, workspace: workspace)
      page  = described_class.for(workspace, user, today: Date.new(2024, 3, 15))
      first = page.needs_you.first
      expect(first.kind).to eq(:reconcile_statements)
      expect(first.payload[:count]).to eq(1)
      expect(page.needs_you.map(&:kind)).not_to include(:add_statement)
    end
  end
end
