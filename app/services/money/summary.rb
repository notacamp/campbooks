# frozen_string_literal: true

class Money
  # The data behind Scout's one-paragraph read on the Money page: what you're owed,
  # what you owe, what's late, and the renewals to decide. It answers only what the
  # obligations prove — the sums, the counts, the worst-late item each way, the next
  # week's due items — and NEVER claims solvency: "the month is fine" is not
  # computable without a bank balance, so the strongest closing it offers is
  # `nothing_else_this_month?` ("Nothing else is due this month.").
  #
  # The `Campbooks::Money::Summary` component turns this data into the sentence
  # (bold amounts, pluralized, localized).
  class Summary
    def self.for(workspace, user, today: Date.current, ledger: nil)
      new(workspace, user, today, ledger)
    end

    def initialize(workspace, user, today = Date.current, ledger = nil)
      @workspace = workspace
      @user = user
      @today = today
      @ledger = ledger || Money::Ledger.for(workspace, user, today: today)
    end

    def any? = @ledger.any?

    # Ordered [Money] per currency, the workspace currency first.
    def owed_to_you = ordered_money(@ledger.owed_to_you_by_currency)
    def you_owe     = ordered_money(@ledger.you_owe_by_currency)

    # The headline figure each way (primary currency); nil when nothing is open.
    def owed_to_you_total = owed_to_you.first
    def you_owe_total     = you_owe.first

    def owed_to_you? = owed_to_you.any?
    def you_owe?     = you_owe.any?

    # Count of open items in each direction (late + due), for "across N bills".
    def owe_count  = open(:payable).size
    def owed_count = open(:receivable).size

    def late_receivable = @ledger.late.select(&:receivable?).max_by(&:amount_cents)
    def late_payable    = @ledger.late.select(&:payable?).max_by(&:amount_cents)

    # The highest-priority open obligation (late or due), regardless of direction.
    # Used by the summary component to name "the one that matters".
    def matters_most = @ledger.most_pressing

    def late_receivable_count = @ledger.late.count(&:receivable?)
    def late_payable_count    = @ledger.late.count(&:payable?)

    # Renewals awaiting a keep/cancel decision.
    def renewals = @ledger.obligations.select(&:decide?)

    # Open due items landing within the next week.
    def due_next_7
      @ledger.due.select { |o| o.due? && o.due_on && o.due_on.between?(@today, @today + 7.days) }
    end

    # True when no further due/decide obligation lands before the month is out — the
    # only honest "you're clear for now" the page will make.
    def nothing_else_this_month?
      month_end = @today.end_of_month
      @ledger.obligations.none? do |o|
        (o.due? || o.decide?) && o.due_on && o.due_on > @today && o.due_on <= month_end
      end
    end

    def primary_currency
      @primary_currency ||=
        @workspace.try(:default_currency).presence ||
        @workspace.try(:currency).presence ||
        ::Money.default_currency.iso_code
    end

    private

    def open(direction)
      @ledger.open_obligations.select { |o| o.direction == direction }
    end

    def ordered_money(by_currency)
      by_currency
        .sort_by { |currency, _cents| currency == primary_currency ? 0 : 1 }
        .map { |currency, cents| ::Money.new(cents, currency) }
    end
  end
end
