# frozen_string_literal: true

class Money
  # Recognises subscriptions. Repeated money documents from one counterpart, in one
  # direction, that arrive on a monthly or yearly beat for a steady amount, are a
  # recurring line — Cloudhost's "July invoice" is really "July invoice · subscription".
  # Pure: it reads a set of documents and answers, per counterpart+direction, whether
  # that pairing recurs, on what cadence, and when the next one is expected (the Time
  # page — PR 6 — reads `next_renewal_on`; here it only tags the row).
  #
  #   rec = Money::Recurrence.for(workspace.documents.money_types)
  #   rec[Money::Recurrence.group_key(doc)]   # => Info(recurring:, cadence:, next_renewal_on:) or nil
  class Recurrence
    Info = Struct.new(:recurring, :cadence, :next_renewal_on, keyword_init: true) do
      def recurring? = recurring == true
    end

    MONTHLY_DAYS = (28..33)
    YEARLY_DAYS  = (350..380)
    AMOUNT_TOLERANCE = 0.15 # the spread across the series must stay within 15%
    MIN_OCCURRENCES  = 3

    def self.for(documents, today: Date.current)
      new(documents, today).call
    end

    # The grouping key for a document: normalised counterpart + money direction.
    # Returns nil for a non-money document (no direction).
    def self.group_key(document)
      direction = document.direction
      return nil unless direction

      name = document.entity_display_name.to_s.downcase.strip.gsub(/\s+/, " ")
      return nil if name.empty?

      [ name, direction ]
    end

    def initialize(documents, today = Date.current)
      @documents = documents
      @today = today
    end

    # => { [counterpart, direction] => Info } for the groups that qualify.
    def call
      grouped.each_with_object({}) do |(key, docs), acc|
        info = detect(docs)
        acc[key] = info if info
      end
    end

    private

    def grouped
      @documents.group_by { |doc| self.class.group_key(doc) }.tap { |h| h.delete(nil) }
    end

    def detect(docs)
      dated = docs.filter_map { |d| dated_pair(d) }.sort_by(&:first)
      return nil if dated.size < MIN_OCCURRENCES
      return nil unless amounts_steady?(dated.map(&:last))

      cadence = cadence_for(dated.map(&:first))
      return nil unless cadence

      last_date = dated.last.first
      Info.new(recurring: true, cadence: cadence, next_renewal_on: advance(last_date, cadence))
    end

    # [date, amount_cents] for a document with both, else nil.
    def dated_pair(doc)
      date = anchor_date(doc)
      cents = doc.amount_cents
      return nil unless date && cents && cents.positive?

      [ date, cents ]
    end

    def anchor_date(doc)
      value = doc.due_date || doc.document_date
      value.respond_to?(:to_date) ? value.to_date : nil
    rescue StandardError
      nil
    end

    def amounts_steady?(cents_list)
      max = cents_list.max
      min = cents_list.min
      return false if max.to_i.zero?

      (max - min).to_f / max <= AMOUNT_TOLERANCE
    end

    # Every consecutive gap must sit in the same monthly-or-yearly band.
    def cadence_for(dates)
      gaps = dates.each_cons(2).map { |a, b| (b - a).to_i }
      return :monthly if gaps.all? { |g| MONTHLY_DAYS.include?(g) }
      return :yearly  if gaps.all? { |g| YEARLY_DAYS.include?(g) }

      nil
    end

    def advance(date, cadence)
      cadence == :yearly ? date + 1.year : date + 1.month
    end
  end
end
