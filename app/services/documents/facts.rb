# frozen_string_literal: true

module Documents
  # "What the document says", as Scout read it — the SCOUT READ column on Paper. A
  # sequence of fact segments (kind first, then the amount, then the date that matters)
  # built ONLY from the extraction schema's fields; it never invents a fact and never
  # raises on empty metadata.
  #
  #   Documents::Facts.for(doc).to_s        # "Invoice · €248.00 · due Aug 14"
  #   Documents::Facts.for(doc).segments    # [Segment(kind), Segment(amount, emphasis), Segment(date)]
  #
  # The row renders the segments joined by " · " and emphasises the amount.
  class Facts
    include Documents::MatterDate

    Segment = Struct.new(:text, :emphasis, keyword_init: true)

    def self.for(document, today: Date.current)
      new(document, today)
    end

    def initialize(document, today = Date.current)
      @doc = document
      @today = today
    end

    def segments
      @segments ||= ([ Segment.new(text: kind_label, emphasis: false) ] + type_segments)
                    .select { |s| s.text.present? }
    end

    def to_s
      segments.map(&:text).join(" · ")
    end

    private

    def kind_label
      I18n.t("paper.kinds.#{@doc.document_type}", default: @doc.document_type.to_s.humanize)
    end

    def type_segments
      case @doc.document_type
      when "expense_invoice", "revenue_invoice", "credit_note", "receipt"
        money_segments
      when "contract"
        contract_segments
      when "insurance_policy", "certificate", "identification", "vehicle_document"
        renewal_segments
      when "bank_statement"
        statement_segments
      else
        # Anything else: surface an amount if the schema captured one, nothing invented.
        [ amount_segment ].compact
      end
    end

    def money_segments
      [ amount_segment, due_segment ].compact
    end

    def amount_segment
      return nil if @doc.amount_cents.blank?

      Segment.new(text: format_currency(@doc.amount_cents, @doc.currency), emphasis: true)
    end

    def due_segment
      due = date_field(:due_date)
      return nil unless due

      Segment.new(text: I18n.t("paper.facts.due", date: matter_date(due)), emphasis: false)
    end

    def contract_segments
      [ term_segment, ends_segment ].compact
    end

    # "12 months" when both ends of the term are known — computed, not stored.
    def term_segment
      start_on = date_field(:period_start)
      end_on   = date_field(:period_end)
      return nil unless start_on && end_on && end_on > start_on

      months = (end_on.year * 12 + end_on.month) - (start_on.year * 12 + start_on.month)
      return nil if months <= 0

      Segment.new(text: I18n.t("paper.facts.months", count: months), emphasis: false)
    end

    def ends_segment
      end_on = date_field(:period_end) || reminder_date
      return nil unless end_on

      Segment.new(text: I18n.t("paper.facts.ends", date: matter_date(end_on)), emphasis: false)
    end

    def renewal_segments
      date = date_field(:period_end) || reminder_date
      return [] unless date

      [ Segment.new(text: I18n.t("paper.facts.renews", date: matter_date(date)), emphasis: false) ]
    end

    def statement_segments
      reconciliation = @doc.reconciliations_as_statement.min_by(&:created_at)
      return [] unless reconciliation

      total   = reconciliation.bank_transactions.size
      matched = reconciliation.bank_transactions.count { |t| t.status == "matched" }
      [
        Segment.new(text: I18n.t("paper.facts.transactions", count: total), emphasis: false),
        Segment.new(text: I18n.t("paper.facts.matched", count: matched), emphasis: false)
      ]
    end

    def reminder_date
      Reminder.where(source: @doc, reminder_type: %i[renewal deadline])
              .where.not(status: :dismissed)
              .minimum(:due_at)&.to_date
    end

    def format_currency(cents, currency)
      Money.new(cents, currency).format
    rescue StandardError
      nil
    end

    def date_field(attr)
      value = @doc.public_send(attr)
      value.respond_to?(:to_date) ? value.to_date : nil
    rescue StandardError
      nil
    end
  end
end
