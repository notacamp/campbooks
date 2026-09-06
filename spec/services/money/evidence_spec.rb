# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::Evidence do
  let(:workspace) { create(:workspace) }

  def ready_statement(period_start:, period_end:)
    create(:reconciliation, :ready, :with_bank,
           workspace: workspace,
           period_start: period_start,
           period_end: period_end)
  end

  def doc(**attrs)
    create(:document, :approved, workspace: workspace,
           document_type: :expense_invoice,
           currency: "EUR",
           amount_cents: 10_000,
           **attrs)
  end

  def evidence
    described_class.for(workspace)
  end

  describe "#any? / #statements" do
    it "returns false when there are no ready statements" do
      create(:reconciliation, workspace: workspace, status: :pending,
             period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
      expect(evidence.any?).to be false
    end

    it "returns true when there is at least one ready statement with a period" do
      ready_statement(period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
      expect(evidence.any?).to be true
    end

    it "excludes ready statements missing period_start or period_end" do
      create(:reconciliation, :ready, workspace: workspace, period_start: nil, period_end: nil)
      expect(evidence.any?).to be false
    end
  end

  describe "#status_for" do
    context "when the document is settled" do
      it "returns :settled regardless of statement coverage" do
        stmt = ready_statement(period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
        d = doc(document_date: Date.new(2024, 1, 10),
                settled_at: Time.current, settled_source: "manual")
        expect(evidence.status_for(d)).to eq(:settled)
      end
    end

    context "when there are no ready statements" do
      it "returns :unconfirmed" do
        d = doc(document_date: Date.new(2024, 1, 10))
        expect(evidence.status_for(d)).to eq(:unconfirmed)
      end
    end

    context "with a ready statement" do
      let(:period_end) { Date.new(2024, 1, 31) }

      before do
        ready_statement(period_start: Date.new(2024, 1, 1), period_end: period_end)
      end

      it "returns :missing when latest_covered_on >= anchor + 7" do
        # anchor = Jan 24; period_end Jan 31 >= Jan 24 + 7 = Jan 31 => exactly on boundary
        d = doc(document_date: Date.new(2024, 1, 24), due_date: nil)
        expect(evidence.status_for(d)).to eq(:missing)
      end

      it "returns :unconfirmed at anchor + 6 (not yet covered)" do
        # anchor = Jan 25; period_end Jan 31 < Jan 25 + 7 = Feb 1
        d = doc(document_date: Date.new(2024, 1, 25), due_date: nil)
        expect(evidence.status_for(d)).to eq(:unconfirmed)
      end

      it "prefers due_date over document_date as the anchor" do
        # due_date = Jan 20 -> covered (Jan 31 >= Jan 27); document_date = Jan 28 -> not covered
        d = doc(document_date: Date.new(2024, 1, 28), due_date: Date.new(2024, 1, 20))
        expect(evidence.status_for(d)).to eq(:missing)
      end

      it "falls back to document_date when due_date is nil" do
        d = doc(document_date: Date.new(2024, 1, 10), due_date: nil)
        expect(evidence.status_for(d)).to eq(:missing)
      end
    end

    context "when there are non-ready reconciliations" do
      it "ignores pending/failed reconciliations" do
        create(:reconciliation, workspace: workspace, status: :pending,
               period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
        d = doc(document_date: Date.new(2024, 1, 10))
        expect(evidence.status_for(d)).to eq(:unconfirmed)
      end
    end
  end

  describe "#anchor_for" do
    it "returns due_date when present" do
      d = doc(due_date: Date.new(2024, 2, 15), document_date: Date.new(2024, 2, 1))
      expect(evidence.anchor_for(d)).to eq(Date.new(2024, 2, 15))
    end

    it "returns document_date when due_date is nil" do
      d = doc(due_date: nil, document_date: Date.new(2024, 2, 1))
      expect(evidence.anchor_for(d)).to eq(Date.new(2024, 2, 1))
    end

    it "returns nil when both dates are nil" do
      d = doc(due_date: nil, document_date: nil)
      expect(evidence.anchor_for(d)).to be_nil
    end
  end

  describe "#statement_for" do
    let(:jan) { ready_statement(period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31)) }
    let(:feb) { ready_statement(period_start: Date.new(2024, 2, 1), period_end: Date.new(2024, 2, 29)) }

    before { jan; feb }

    it "returns the containing statement when anchor is inside a period" do
      d = doc(document_date: Date.new(2024, 1, 15), due_date: nil)
      expect(evidence.statement_for(d)).to eq(jan)
    end

    it "returns the earliest next statement when anchor is before any period" do
      d = doc(document_date: Date.new(2023, 12, 31), due_date: nil)
      expect(evidence.statement_for(d)).to eq(jan)
    end

    it "returns nil when anchor is after all periods" do
      d = doc(document_date: Date.new(2024, 3, 1), due_date: nil)
      expect(evidence.statement_for(d)).to be_nil
    end

    it "returns nil when document has no anchor" do
      d = doc(document_date: nil, due_date: nil)
      expect(evidence.statement_for(d)).to be_nil
    end
  end

  describe "#label_for" do
    it "returns the month name for a single-month statement" do
      stmt = ready_statement(period_start: Date.new(2024, 8, 1), period_end: Date.new(2024, 8, 31))
      expect(evidence.label_for(stmt)).to eq("August")
    end

    it "returns period_label for a multi-month statement" do
      stmt = ready_statement(period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 3, 31))
      expect(evidence.label_for(stmt)).to eq(stmt.period_label)
    end
  end
end
