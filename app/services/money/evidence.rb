# frozen_string_literal: true

class Money
  # What the bank has proven. The only source of "not on a statement".
  #
  # A document is :missing only once a ready (reconciled) statement covers its
  # anchor date (due_date || document_date) with a 7-day grace and shows no
  # confirmed match. Until then we say nothing: no "late", no totals, no due dates.
  #
  #   evidence = Money::Evidence.for(workspace)
  #   evidence.status_for(doc)          # :settled | :missing | :unconfirmed
  #   evidence.statement_for(doc)       # Reconciliation or nil
  class Evidence
    GRACE_DAYS = 7

    def self.for(workspace) = new(workspace)

    def initialize(workspace)
      @workspace = workspace
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

    # The date anchor for a document: due_date first, then document_date, as Date.
    def anchor_for(doc)
      safe_date(doc.due_date) || safe_date(doc.document_date)
    rescue StandardError
      nil
    end

    # Status for a document:
    #   :settled     if doc.settled?
    #   :missing     if anchor is covered by latest_covered_on + GRACE_DAYS and
    #                no confirmed bank match
    #   :unconfirmed otherwise (no statement yet, or statement does not reach the anchor)
    def status_for(doc)
      return :settled if doc.settled?

      anchor = anchor_for(doc)
      return :unconfirmed unless anchor && latest_covered_on

      if latest_covered_on >= anchor + GRACE_DAYS
        :missing
      else
        :unconfirmed
      end
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

    # Human label for a statement.
    # "August" when period_start and period_end share a month, else statement.period_label.
    def label_for(statement)
      return statement.period_label if statement.period_start.blank? || statement.period_end.blank?

      if statement.period_start.year  == statement.period_end.year &&
         statement.period_start.month == statement.period_end.month
        I18n.l(statement.period_start, format: :month_name)
      else
        statement.period_label
      end
    end

    private

    def safe_date(value)
      value.respond_to?(:to_date) ? value.to_date : nil
    rescue StandardError
      nil
    end
  end
end
