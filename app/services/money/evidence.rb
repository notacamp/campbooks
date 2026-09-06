# frozen_string_literal: true

class Money
  # What the bank has proven. The only source of "not on a statement", and the
  # calendar Money is organised around: months, each reconciled or not.
  #
  # A document is :missing only when a ready (reconciled) statement covers its
  # anchor date (due_date || document_date) AND the week after it, and no bank
  # line points at it. A month nobody has reconciled proves nothing, however many
  # newer statements exist. Until then we say nothing: no "late", no totals.
  #
  #   evidence = Money::Evidence.for(workspace)
  #   evidence.status_for(doc)          # :settled | :pending | :missing | :unconfirmed
  #   evidence.statement_for(doc)       # Reconciliation or nil
  #   evidence.month_to_reconcile       # the most recent completed month (a Date, day 1)
  #   evidence.months                   # Month slots, newest first, from the first statement
  class Evidence
    GRACE_DAYS   = 7
    HALF_MONTH   = 15
    MONTHS_LIMIT = 120 # ten years back; the pills fold the rest behind "Earlier"

    # One calendar month on the timeline and the ready statements that cover it.
    Month = Struct.new(:starts_on, :statements, keyword_init: true) do
      def reconciled? = statements.any?
      def ends_on     = starts_on.end_of_month
    end

    def self.for(workspace, today: Date.current) = new(workspace, today: today)

    def initialize(workspace, today: Date.current)
      @workspace = workspace
      @today     = today
    end

    # Ready reconciliations with both period_start and period_end, newest period_end first.
    def statements
      @statements ||= @workspace.reconciliations
                                .where(status: :ready)
                                .where.not(period_start: nil)
                                .where.not(period_end: nil)
                                .order(period_end: :desc)
                                .includes(:statement_document)
    end

    def any? = statements.any?

    # Maximum period_end across all ready statements, or nil.
    def latest_covered_on
      @latest_covered_on ||= statements.first&.period_end
    end

    # Is `date` inside a reconciled statement's period?
    def covered?(date)
      periods.any? { |from, to| from <= date && date <= to }
    end

    # The date anchor for a document: due_date first, then document_date, as Date.
    def anchor_for(doc)
      safe_date(doc.due_date) || safe_date(doc.document_date)
    rescue StandardError
      nil
    end

    # Status for a document:
    #   :settled     if doc.settled?
    #   :pending     a bank line already points at it (a suggested match waiting for
    #                a look, or a confirmed partial payment): it has its line, so it is
    #                never "missing"; Needs-you carries it instead
    #   :missing     a reconciled statement covers the anchor and the GRACE_DAYS after
    #                it, and no bank line points at it
    #   :unconfirmed otherwise (no statement covers that month yet)
    def status_for(doc)
      return :settled if doc.settled?
      return :pending if pending_document_ids.include?(doc.id)

      anchor = anchor_for(doc)
      return :unconfirmed unless anchor

      covered?(anchor) && covered?(anchor + GRACE_DAYS) ? :missing : :unconfirmed
    end

    # The statement whose period contains the anchor, else the earliest statement
    # with period_start > anchor, else nil.
    def statement_for(doc)
      anchor = anchor_for(doc)
      return nil unless anchor

      containing = statements.find { |s| s.period_start <= anchor && s.period_end >= anchor }
      return containing if containing

      statements.select { |s| s.period_start > anchor }.min_by(&:period_start)
    end

    # ── Months ───────────────────────────────────────────────────────────────

    # The most recent completed month: the one whose statement is due.
    def month_to_reconcile
      @month_to_reconcile ||= @today.prev_month.beginning_of_month
    end

    # Ready statements that reconcile a month (see #months_of).
    def statements_covering(month_start)
      statements.select { |s| months_of(s).include?(month_start) }
    end

    def month_reconciled?(month_start)
      statements_covering(month_start).any?
    end

    # The months a statement reconciles. Periods come off the statement's own
    # dates (often the first and last transaction, so "2 Jul to 29 Jul" is July),
    # and a bank's cycle may straddle months ("15 Jul to 14 Aug"): a statement
    # inside one month is that month; a longer one reconciles every month it
    # overlaps by at least HALF_MONTH days; one too short for either takes the
    # month it overlaps most.
    def months_of(statement)
      (@months_of ||= {})[statement.id] ||= compute_months_of(statement)
    end

    # Every month from the first reconciled one to the month to reconcile,
    # newest first, capped at MONTHS_LIMIT. With no statements: just the month to
    # reconcile.
    def months
      @months ||= begin
        first = statements.flat_map { |s| months_of(s) }.min || month_to_reconcile
        first = [ first, month_to_reconcile << (MONTHS_LIMIT - 1) ].max
        list  = []
        cursor = month_to_reconcile
        while cursor >= first
          list << Month.new(starts_on: cursor, statements: statements_covering(cursor))
          cursor = cursor.prev_month
        end
        list
      end
    end

    def gap_months
      months.reject(&:reconciled?)
    end

    # ── Labels ───────────────────────────────────────────────────────────────

    # "August", or "November 2025" when it isn't this year.
    def month_label(date)
      name = I18n.l(date, format: :month_name)
      date.year == @today.year ? name : "#{name} #{date.year}"
    end

    # A statement's label: its month (with the year when needed) when it sits
    # inside one month, else its period.
    def label_for(statement)
      return statement.period_label if statement.period_start.blank? || statement.period_end.blank?

      if statement.period_start.year  == statement.period_end.year &&
         statement.period_start.month == statement.period_end.month
        month_label(statement.period_start)
      else
        statement.period_label
      end
    end

    private

    def periods
      @periods ||= statements.map { |s| [ s.period_start, s.period_end ] }
    end

    def compute_months_of(statement)
      from = statement.period_start.beginning_of_month
      to   = statement.period_end.beginning_of_month
      return [ from ] if from == to

      candidates = []
      cursor = from
      while cursor <= to
        candidates << [ cursor, overlap_days(statement, cursor) ]
        cursor = cursor.next_month
      end
      covered = candidates.select { |_, days| days >= HALF_MONTH }.map(&:first)
      covered.presence || [ candidates.max_by { |month, days| [ days, month ] }.first ]
    end

    def overlap_days(statement, month_start)
      from = [ statement.period_start, month_start ].max
      to   = [ statement.period_end, month_start.end_of_month ].min
      from > to ? 0 : (to - from).to_i + 1
    end

    # Documents a bank line in this workspace already points at (suggested or
    # confirmed, never rejected). One query, memoized for the request.
    def pending_document_ids
      @pending_document_ids ||= TransactionMatch
                                  .where(status: %i[suggested confirmed])
                                  .joins(:bank_transaction)
                                  .where(bank_transactions: { workspace_id: @workspace.id })
                                  .distinct
                                  .pluck(:document_id)
                                  .to_set
    end

    def safe_date(value)
      value.respond_to?(:to_date) ? value.to_date : nil
    rescue StandardError
      nil
    end
  end
end
