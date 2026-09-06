# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::Evidence do
  let(:workspace) { create(:workspace) }
  let(:today)     { Date.new(2024, 2, 15) }

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

  def evidence(today: self.today)
    described_class.for(workspace, today: today)
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
        ready_statement(period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
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

      it "returns :missing when the statement covers the anchor and the week after it" do
        # anchor = Jan 24; Jan 24 + 7 = Jan 31, the last day covered
        d = doc(document_date: Date.new(2024, 1, 24), due_date: nil)
        expect(evidence.status_for(d)).to eq(:missing)
      end

      it "returns :unconfirmed when the week after the anchor runs past the statement" do
        # anchor = Jan 25; Jan 25 + 7 = Feb 1, not covered
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

      it "returns :pending when a bank line already points at the document (suggested match)" do
        d = doc(document_date: Date.new(2024, 1, 10), due_date: nil)
        txn = create(:bank_transaction, reconciliation: Reconciliation.last, workspace: workspace,
                     status: :suggested, amount_cents: -10_000, booked_on: Date.new(2024, 1, 12))
        txn.transaction_matches.create!(document: d, status: :suggested, matched_by: :ai, confidence: 0.8,
                                        match_reasons: {})
        expect(evidence.status_for(d)).to eq(:pending)
      end

      it "returns :pending for a confirmed partial payment" do
        d = doc(document_date: Date.new(2024, 1, 10), due_date: nil, amount_cents: 150_000)
        txn = create(:bank_transaction, reconciliation: Reconciliation.last, workspace: workspace,
                     status: :matched, amount_cents: -60_000, booked_on: Date.new(2024, 1, 12))
        txn.transaction_matches.create!(document: d, status: :confirmed, matched_by: :manual, confidence: 1.0,
                                        allocated_cents: 60_000, match_reasons: {})
        expect(d.reload).not_to be_settled
        expect(evidence.status_for(d)).to eq(:pending)
      end

      it "ignores a rejected match" do
        d = doc(document_date: Date.new(2024, 1, 10), due_date: nil)
        txn = create(:bank_transaction, reconciliation: Reconciliation.last, workspace: workspace,
                     status: :unmatched, amount_cents: -10_000, booked_on: Date.new(2024, 1, 12))
        txn.transaction_matches.create!(document: d, status: :rejected, matched_by: :ai, confidence: 0.5,
                                        match_reasons: {})
        expect(evidence.status_for(d)).to eq(:missing)
      end
    end

    context "with a gap between statements" do
      before do
        ready_statement(period_start: Date.new(2025, 7, 1), period_end: Date.new(2025, 7, 31))
        ready_statement(period_start: Date.new(2025, 11, 1), period_end: Date.new(2025, 11, 30))
      end

      it "says nothing about an invoice dated in a month nobody reconciled, however new the newest statement is" do
        d = doc(document_date: Date.new(2025, 9, 10), due_date: nil)
        expect(evidence(today: Date.new(2026, 9, 6)).status_for(d)).to eq(:unconfirmed)
      end

      it "still marks an invoice from a reconciled month missing" do
        d = doc(document_date: Date.new(2025, 11, 10), due_date: nil)
        expect(evidence(today: Date.new(2026, 9, 6)).status_for(d)).to eq(:missing)
      end

      it "does not stretch one statement's coverage into the next when the week after crosses a gap" do
        d = doc(document_date: Date.new(2025, 7, 28), due_date: nil)
        expect(evidence(today: Date.new(2026, 9, 6)).status_for(d)).to eq(:unconfirmed)
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

  describe "months" do
    let(:today) { Date.new(2026, 9, 6) }

    it "is the most recent completed month that wants reconciling" do
      expect(evidence.month_to_reconcile).to eq(Date.new(2026, 8, 1))
    end

    it "lists just that month when nothing is reconciled yet" do
      months = evidence.months
      expect(months.map(&:starts_on)).to eq([ Date.new(2026, 8, 1) ])
      expect(months.first).not_to be_reconciled
    end

    context "with statements for June, July (twice) and November 2025" do
      let!(:jun)   { ready_statement(period_start: Date.new(2025, 6, 1), period_end: Date.new(2025, 6, 30)) }
      let!(:jul_a) { ready_statement(period_start: Date.new(2025, 7, 1), period_end: Date.new(2025, 7, 31)) }
      let!(:jul_b) { ready_statement(period_start: Date.new(2025, 7, 1), period_end: Date.new(2025, 7, 31)) }
      let!(:nov)   { ready_statement(period_start: Date.new(2025, 11, 1), period_end: Date.new(2025, 11, 30)) }

      it "runs from the month to reconcile back to the first statement, newest first" do
        months = evidence.months
        expect(months.size).to eq(15)
        expect(months.first.starts_on).to eq(Date.new(2026, 8, 1))
        expect(months.last.starts_on).to eq(Date.new(2025, 6, 1))
        expect(months.map(&:starts_on)).to eq(months.map(&:starts_on).sort.reverse)
      end

      it "knows which months are reconciled and by what" do
        by_month = evidence.months.index_by(&:starts_on)
        expect(by_month[Date.new(2026, 8, 1)]).not_to be_reconciled
        expect(by_month[Date.new(2025, 11, 1)].statements).to eq([ nov ])
        expect(by_month[Date.new(2025, 7, 1)].statements).to match_array([ jul_a, jul_b ])
        expect(by_month[Date.new(2025, 6, 1)].statements).to eq([ jun ])
        expect(evidence.month_reconciled?(Date.new(2025, 9, 1))).to be false
      end

      it "counts the months without a statement" do
        expect(evidence.gap_months.size).to eq(12)
      end
    end

    describe "which months a statement reconciles" do
      def months_of(from, to)
        stmt = ready_statement(period_start: from, period_end: to)
        evidence.months_of(stmt)
      end

      it "is its month when the period sits inside one, even off the transaction dates" do
        expect(months_of(Date.new(2026, 7, 2), Date.new(2026, 7, 28))).to eq([ Date.new(2026, 7, 1) ])
      end

      it "is every month it overlaps by half a month or more" do
        expect(months_of(Date.new(2026, 1, 1), Date.new(2026, 3, 31)))
          .to eq([ Date.new(2026, 1, 1), Date.new(2026, 2, 1), Date.new(2026, 3, 1) ])
      end

      it "gives a mid-month cycle to the month it mostly falls in" do
        expect(months_of(Date.new(2026, 7, 15), Date.new(2026, 8, 14))).to eq([ Date.new(2026, 7, 1) ])
        expect(months_of(Date.new(2026, 7, 20), Date.new(2026, 8, 31))).to eq([ Date.new(2026, 8, 1) ])
      end

      it "falls back to the month it overlaps most when it is too short for either" do
        expect(months_of(Date.new(2026, 7, 28), Date.new(2026, 8, 8))).to eq([ Date.new(2026, 8, 1) ])
      end

      it "keeps a statement ending before the month's last day on the timeline" do
        stmt = ready_statement(period_start: Date.new(2026, 7, 2), period_end: Date.new(2026, 7, 28))
        expect(evidence.months.find { |m| m.starts_on == Date.new(2026, 7, 1) }.statements).to eq([ stmt ])
      end
    end
  end

  describe "labels" do
    it "names a month alone when it is this year" do
      expect(evidence(today: Date.new(2024, 9, 15)).month_label(Date.new(2024, 8, 1))).to eq("August")
    end

    it "adds the year when it isn't this year" do
      expect(evidence(today: Date.new(2026, 9, 6)).month_label(Date.new(2025, 11, 1))).to eq("November 2025")
    end

    it "labels a single-month statement by its month" do
      stmt = ready_statement(period_start: Date.new(2024, 8, 1), period_end: Date.new(2024, 8, 31))
      expect(evidence(today: Date.new(2024, 9, 15)).label_for(stmt)).to eq("August")
      expect(evidence(today: Date.new(2026, 9, 6)).label_for(stmt)).to eq("August 2024")
    end

    it "returns period_label for a multi-month statement" do
      stmt = ready_statement(period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 3, 31))
      expect(evidence.label_for(stmt)).to eq(stmt.period_label)
    end
  end
end
