# frozen_string_literal: true

# Previews for the Money surface's three components — the Scout read, the 30-day
# timeline, and the ledger table — over an in-memory set of obligations mirroring
# the approved mock (no DB, no seeded data). The fake ledger answers just what the
# components read; the summary is the REAL Money::Summary wrapping it.
class MoneyComponentsPreview < ViewComponent::Preview
  # A light-Ember note: owed / owe / late / renewals, bold amounts, honest.
  # @label Scout read
  def scout_read
    render Campbooks::Money::Summary.new(summary: build_summary, class: "max-w-[900px]")
  end

  # Two directions on one axis; late bars destructive + labelled, the overdue
  # region shaded, bars linked to their ledger rows.
  # @label Timeline
  def timeline
    render Campbooks::Money::Timeline.new(ledger: build_ledger, summary: build_summary, today: Date.current)
  end

  # Late / Due / Settled, with status chips (icon + label) and per-row actions.
  # @label Ledger table
  def ledger
    render Campbooks::Money::Ledger.new(ledger: build_ledger)
  end

  private

  FakeLedger = Struct.new(:obligations, :today) do
    def late    = obligations.select(&:late?)
    def due     = obligations.select { |o| o.due? || o.decide? }.sort_by(&:due_on)
    def settled = obligations.select(&:settled?)
    def sections = [ [ :late, late ], [ :due, due ], [ :settled, settled ] ].reject { |(_k, list)| list.empty? }
    def open_obligations = obligations.select { |o| o.late? || o.due? }
    def owed_to_you_by_currency = sum(:receivable)
    def you_owe_by_currency = sum(:payable)
    def range_start = today - 21
    def range_end   = today + 30
    def any? = obligations.any?

    private

    def sum(direction)
      open_obligations.select { |o| o.direction == direction }
                      .each_with_object(Hash.new(0)) { |o, acc| acc[o.currency] += o.amount_cents.to_i }
    end
  end

  def build_ledger(today = Date.current)
    FakeLedger.new(sample_obligations(today), today)
  end

  def build_summary(today = Date.current)
    Money::Summary.for(nil, nil, today: today, ledger: build_ledger(today))
  end

  def sample_obligations(today)
    [
      obligation("doc:brightloop", :receivable, "Brightloop", "Invoice #0231 you sent", 120_000, today - 12, :late, %i[mark_paid send_reminder]),
      obligation("doc:cloudhost", :payable, "Cloudhost", "July invoice · subscription", 24_800, today - 20, :late, %i[mark_paid pay], recurring: true, pay_url: "https://pay.example.com/x"),
      obligation("doc:staples", :payable, "Staples Portugal", "Order 4471", 36_400, today + 5, :due, %i[mark_paid pay], pay_url: "https://pay.example.com/y"),
      obligation("doc:acme", :receivable, "Acme Consulting", "Invoice #0234 you sent", 222_000, today + 12, :due, %i[remind_on]),
      obligation("rem:seguro", :payable, "Seguro Renovação", "Policy renewal · yearly", 41_200, today + 28, :decide, %i[keep cancel]),
      obligation("doc:lumen", :receivable, "Lumen Studio", "Invoice #0233 you sent", 64_000, today - 2, :settled, [], settled_on: today - 2, settled_via: "Millennium BCP · line 14"),
      obligation("doc:edp", :payable, "EDP", "August bill · subscription", 9_640, today - 6, :settled, [], settled_on: today - 6, settled_via: "Millennium BCP · line 9")
    ]
  end

  def obligation(id, direction, counterpart, what, cents, due, status, actions, recurring: false, pay_url: nil, settled_on: nil, settled_via: nil)
    Money::Obligation.new(
      id: id, direction: direction, counterpart: counterpart, what: what,
      amount: ::Money.new(cents, "EUR"), due_on: due, status: status,
      settled_on: settled_on, settled_via: settled_via, source_email_message: nil,
      document: nil, reminder: nil, recurring: recurring, cadence: nil,
      next_renewal_on: nil, due_estimated: false, pay_url: pay_url, actions: actions
    )
  end
end
