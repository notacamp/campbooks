# frozen_string_literal: true

module Documents
  # The derived, meaning-bearing status of a document for Paper (the "STATUS" column
  # and the Money surface). A file browser shows "approved / pending"; Paper says what
  # the document actually IS right now — unpaid, late, paid, signed, expiring, needs
  # review — derived from the extraction schema (amount + dates), the review state, and
  # the accounting reconciliation. Pure and side-effect-free: pass `today:` to test the
  # boundaries.
  #
  # Precedence (first match wins):
  #   1. AI is still working / broke        → processing / failed
  #   2. awaiting human sign-off            → needs_review   (a status AND a Now card)
  #   3. money document (amount + due date) → paid / late / unpaid   (settled wins over late)
  #   4. contract / policy / cert / id / vehicle → expired / expiring / signed / filed
  #   5. bank statement with a ready reconciliation → reconciled
  #   6. everything else                    → filed
  #
  # `tone` follows the approved Paper mock (warning for unpaid/late/expiring, destructive
  # for expired/failed, success for paid/signed, ember for needs_review, muted for
  # reconciled and the rest) — Campbooks::StatusChip maps a tone to the tone-* utilities.
  class Status
    include Documents::MatterDate

    # Document types whose "status" is about expiry, not payment.
    EXPIRY_TYPES = %w[contract insurance_policy certificate identification vehicle_document].freeze
    # A dated thing "expiring" this many days out gets a warning rather than "signed/filed".
    EXPIRING_WINDOW_DAYS = 30

    Result = Struct.new(:status, :tone, :label, :detail, :spark, keyword_init: true) do
      # The full chip text: "Unpaid · 20 days", "Paid Aug 28", "Needs review".
      def chip_text
        detail.present? ? "#{label} · #{detail}" : label
      end

      def spark? = spark == true
    end

    def self.for(document, today: Date.current)
      new(document, today).result
    end

    def initialize(document, today = Date.current)
      @doc = document
      @today = today
    end

    def result
      return build(:processing, :muted) if @doc.ai_processing?
      return build(:failed, :destructive) if @doc.ai_failed?
      return build(:needs_review, :ember, spark: true) if needs_review?

      money_result || expiry_result || statement_result || build(:filed, :muted)
    end

    private

    def needs_review?
      @doc.review_pending? && @doc.ai_completed?
    end

    def money_result
      return nil unless @doc.document_type.in?(Document::MONEY_TYPES)
      return nil if @doc.amount_cents.blank?

      if @doc.settled?
        return build(:paid, :success,
                     label: I18n.t("paper.status.paid", date: matter_date(@doc.settled_at.to_date)))
      end

      due = date_field(:due_date)
      # A money document with an amount but no due date (many receipts) isn't an open
      # obligation — it's just filed.
      return build(:filed, :muted) if due.nil?

      if due < @today
        build(:late, :warning, detail: I18n.t("paper.status.days", count: (@today - due).to_i))
      else
        build(:unpaid, :warning)
      end
    end

    def expiry_result
      return nil unless @doc.document_type.in?(EXPIRY_TYPES)

      expiry = expiry_date
      if expiry
        return build(:expired, :destructive) if expiry < @today
        return build(:expiring, :warning, detail: matter_date(expiry)) if expiry <= @today + EXPIRING_WINDOW_DAYS
      end

      # No live expiry: a contract you've signed reads "signed"; the rest are "filed".
      @doc.contract? ? build(:signed, :success) : build(:filed, :muted)
    end

    def statement_result
      return nil unless @doc.bank_statement?

      reconciled = @doc.reconciliations_as_statement.any?(&:ready?)
      reconciled ? build(:reconciled, :muted) : build(:filed, :muted)
    end

    # The end/expiry date for an expiry-type document: the extracted period_end, else the
    # nearest renewal/deadline reminder linked to the document.
    def expiry_date
      date_field(:period_end) || reminder_expiry
    end

    def reminder_expiry
      Reminder.where(source: @doc, reminder_type: %i[renewal deadline])
              .where.not(status: :dismissed)
              .minimum(:due_at)&.to_date
    end

    # A typed date reader that never raises (the :date reader already rescues bad
    # metadata to nil; this stays defensive for anything else).
    def date_field(attr)
      value = @doc.public_send(attr)
      value.respond_to?(:to_date) ? value.to_date : nil
    rescue StandardError
      nil
    end

    def build(status, tone, label: nil, detail: nil, spark: false)
      Result.new(
        status: status,
        tone: tone,
        label: label || I18n.t("paper.status.#{status}"),
        detail: detail,
        spark: spark
      )
    end
  end
end
