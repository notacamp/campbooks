# frozen_string_literal: true

class Money
  # Builds the Money surface's obligations from the substrate Paper already reads:
  # money documents (an amount + a direction + a due date, settled when the bank
  # matched them) and the payment/renewal reminders Scout extracted. It never
  # invents money — every obligation is a Document or a Reminder, so the accounting
  # module stays the source of truth for settlement.
  #
  #   ledger = Money::Ledger.for(workspace, user)
  #   ledger.late      # overdue, both directions
  #   ledger.due       # coming up + renewals to decide, ascending
  #   ledger.settled   # what the bank cleared in the lookback window, newest first
  #   ledger.find("doc:<uuid>")   # one obligation, for a row action
  class Ledger
    DEFAULT_HORIZON  = 30.days
    DEFAULT_LOOKBACK = 45.days
    # The timeline axis reaches this far left of today (overdue region).
    TIMELINE_BACK = 21.days

    # How the ledger's sections are ordered: by date (newest first by default),
    # by amount (largest first) or by counterpart (A–Z). `sections` applies it;
    # `obligations` stays in ascending date order for the timeline and the export.
    SORTS = %i[priority date amount counterpart].freeze
    DIRS  = %i[desc asc].freeze
    DEFAULT_DIRS = { priority: :desc, date: :desc, amount: :desc, counterpart: :asc }.freeze

    def self.for(workspace, user, today: Date.current, horizon: DEFAULT_HORIZON, lookback: DEFAULT_LOOKBACK,
                 sort: :date, dir: nil)
      new(workspace, user, today: today, horizon: horizon, lookback: lookback, sort: sort, dir: dir)
    end

    def self.default_dir(sort) = DEFAULT_DIRS.fetch(sort.to_s.to_sym, :desc)

    attr_reader :today, :horizon, :lookback, :sort, :dir

    def initialize(workspace, user, today:, horizon:, lookback:, sort: :date, dir: nil)
      @workspace = workspace
      @user = user
      @today = today
      @horizon = horizon
      @lookback = lookback
      @sort = SORTS.include?(sort.to_s.to_sym) ? sort.to_s.to_sym : :date
      @dir = DIRS.include?(dir.to_s.to_sym) ? dir.to_s.to_sym : self.class.default_dir(@sort)
    end

    def obligations
      @obligations ||= begin
        list = (document_obligations + reminder_obligations).sort_by { |o| [ o.due_on, o.counterpart.to_s ] }
        enrich_obligations!(list)
        list
      end
    end

    def late
      obligations.select(&:late?)
    end

    def due
      obligations.select { |o| o.due? || o.decide? }.sort_by { |o| [ o.due_on, o.counterpart.to_s ] }
    end

    def settled
      obligations.select(&:settled?).sort_by { |o| o.settled_on || o.due_on }.reverse
    end

    # Ordered, labelled sections for the ledger table, each in the chosen order
    # (newest first by default). Empty sections are dropped.
    def sections
      [ [ :late, sorted(late) ], [ :due, sorted(due) ], [ :settled, sorted(settled) ] ]
        .reject { |(_key, list)| list.empty? }
    end

    def sorted(list)
      if @sort == :priority
        # Settled stays newest-first; open items sort by descending priority, then due date, then counterpart.
        settled, open = list.partition(&:settled?)
        ranked = open.sort_by { |o| [ -(o.priority || 0.0), o.due_on, o.counterpart.to_s.downcase ] }
        return ranked + settled.sort_by { |o| o.settled_on || o.due_on }.reverse
      end

      ordered = list.sort_by { |o| [ sort_key(o), o.counterpart.to_s.downcase, o.id ] }
      @dir == :desc ? ordered.reverse : ordered
    end

    # The highest-priority open late/due obligation, or nil when there are none.
    def most_pressing
      open_obligations.reject(&:settled?).max_by { |o| o.priority || 0.0 }
    end

    def any?
      obligations.any?
    end

    def find(id)
      obligations.find { |o| o.id == id }
    end

    # Open sums per direction (cents per currency), for the timeline lane labels and
    # the summary. Only :late / :due / :decide count as "open".
    def owed_to_you_by_currency
      open_sum(:receivable)
    end

    def you_owe_by_currency
      open_sum(:payable)
    end

    # Timeline axis bounds.
    def range_start = @today - TIMELINE_BACK
    def range_end   = @today + @horizon

    # Renewals to DECIDE are called out on their own — they aren't a committed
    # amount you owe yet (you might cancel), so the open totals cover late + due only.
    def open?(obligation)
      obligation.late? || obligation.due?
    end

    def open_obligations = obligations.select { |o| open?(o) }

    private

    # The date that matters for the row: when it was settled, else when it is due.
    def sort_key(obligation)
      case @sort
      when :amount      then obligation.amount_cents.to_i
      when :counterpart then obligation.counterpart.to_s.downcase
      when :priority    then obligation.priority || 0.0
      else (obligation.settled? ? (obligation.settled_on || obligation.due_on) : obligation.due_on) || Date.new(1970, 1, 1)
      end
    end

    def open_sum(direction)
      open_obligations.select { |o| o.direction == direction }
                      .each_with_object({}) { |o, acc| acc[o.currency] = (acc[o.currency] || 0) + o.amount_cents.to_i }
    end

    # ── Attention enrichment ──────────────────────────────────────────────────
    # Mutates each obligation in place to add: counterpart_weight, attention_reason,
    # amount_ratio, usual_delay_days, priority, and why. One pass, ≤ 3 extra queries.
    def enrich_obligations!(list) # rubocop:disable Metrics/MethodLength
      return if list.empty?

      # 1. Resolve persons / orgs from each obligation's source email contact.
      weights_obj = Attention::Weights.new(@user)
      person_ids = []
      org_ids    = []
      list.each do |o|
        # A document knows its mail two ways: the message it was attached to
        # (email_message, by provider id) or the messages it was linked from
        # (document_email_messages) — the second is the only link for a document
        # that arrived as a link or was matched later, so fall back to it.
        source = o.source_email_message || o.document&.email_messages&.first
        person = source&.contact&.person
        if person
          org = person.primary_organization
          person_ids << person.id
          org_ids << org.id if org
          o.instance_variable_set(:@_person, person)
          o.instance_variable_set(:@_org, org)
        end
      end
      person_rows = weights_obj.persons(person_ids.uniq)
      org_rows    = weights_obj.organizations(org_ids.uniq)

      # 2. Usual amounts and usual delay: group all docs by recurrence key.
      doc_by_key = money_documents.group_by { |d| Money::Recurrence.group_key(d) }
                                  .transform_values { |docs| docs.map(&:amount_cents).compact }
      delay_by_key = money_documents.group_by { |d| Money::Recurrence.group_key(d) }
                                    .transform_values do |docs|
        docs.filter_map do |d|
          next unless d.settled_at && d.due_date

          settled = d.settled_at.to_date rescue nil
          due     = d.due_date.to_date rescue nil
          next unless settled && due

          (settled - due).to_i
        end
      end

      # 3. The ledger's own scale, for a counterpart with no history: an open
      # bill is measured against the median open amount, so a €4,000 invoice from
      # a first-time vendor still outranks an €80 receipt (the "why" line never
      # claims "their usual" for it — that phrase needs the counterpart's history).
      open_amounts = list.reject(&:settled?).filter_map(&:amount_cents).select(&:positive?)
      open_median  = open_amounts.size >= 3 ? median(open_amounts) : nil

      list.each do |o|
        person = o.instance_variable_get(:@_person)
        org    = o.instance_variable_get(:@_org)

        # Counterpart weight: prefer org row, fall back to person row.
        weight_row = (org && org_rows[org.id]) || (person && person_rows[person.id])
        o.counterpart_weight = weight_row&.weight
        # An organization's row leads with "Through <person>" (how it inherited its
        # weight) — on a bill that says nothing, so take the first reason after it.
        o.attention_reason   = weight_row&.reason_values&.find { |r| r.positive? && r.key != "org_lead" }

        # Usual amount and delay — document obligations only.
        if o.document
          key = Money::Recurrence.group_key(o.document)
          if key
            others = doc_by_key[key].to_a - [ o.document.amount_cents ]
            o.amount_ratio = (others.size >= 2 ? o.document.amount_cents.to_f / median(others) : nil)

            delays = delay_by_key[key].to_a
            o.usual_delay_days = delays.size >= 2 ? median(delays).round : nil
          end
        end

        # Priority score (nil for settled).
        unless o.settled?
          days_late  = o.late? ? o.days_late(@today) : 0
          days_until = o.due_on ? (o.due_on - @today).to_i.clamp(0, nil) : nil
          size_ratio = o.amount_ratio
          size_ratio ||= (o.amount_cents.to_f / open_median if open_median && o.amount_cents)
          input = Money::Priority::Input.new(
            status: o.status, days_late: days_late, days_until: days_until,
            amount_ratio: size_ratio, counterpart_weight: o.counterpart_weight,
            usual_delay_days: o.usual_delay_days, payable: o.payable?
          )
          o.priority = Money::Priority.score(input)
        end

        # Why — up to 2 reasons, localized.
        o.why = build_why(o)
      end
    end

    def build_why(o) # rubocop:disable Metrics/CyclomaticComplexity
      reasons = []
      reasons << I18n.t("money.why.times_usual", times: o.amount_ratio.round.to_s) if o.amount_ratio && o.amount_ratio >= 2.0
      if o.payable? && o.usual_delay_days
        if o.usual_delay_days <= 0
          reasons << I18n.t("money.why.pays_on_time")
        elsif o.usual_delay_days > 3
          reasons << I18n.t("money.why.usually_late", count: o.usual_delay_days)
        end
      end
      reasons << o.attention_reason.sentence if o.attention_reason && reasons.size < 2
      reasons << I18n.t("money.why.recurring") if o.recurring? && reasons.empty?
      contact = (o.source_email_message || o.document&.email_messages&.first)&.contact
      reasons << I18n.t("money.why.service") if contact&.kind_service? && o.attention_reason.nil? && reasons.empty?
      reasons.first(2)
    end

    def median(array)
      sorted = array.sort
      mid    = sorted.size / 2
      sorted.size.odd? ? sorted[mid].to_f : (sorted[mid - 1] + sorted[mid]) / 2.0
    end

    # ── Documents ────────────────────────────────────────────────────────────
    def document_obligations
      @document_obligations ||= money_documents.filter_map { |doc| build_document_obligation(doc) }
    end

    def money_documents
      # The enrichment pass walks each document's source message → contact →
      # person → organization; preloaded here so a ledger of hundreds of bills
      # costs four queries for that chain, not four per row.
      @money_documents ||= @workspace.documents.accessible_to(@user).money_types
                                     .includes(email_message: { contact: { person: :primary_organization } },
                                               email_messages: { contact: { person: :primary_organization } })
                                     .reject(&:review_rejected?)
                                     .select { |doc| doc.amount_cents.present? && doc.direction }
    end

    def recurrence
      # Detect series over the counterpart's whole money history, not just the
      # in-window rows (three prior monthly invoices establish a subscription even
      # when only the current one is still open).
      @recurrence ||= Money::Recurrence.for(money_documents, today: @today)
    end

    def build_document_obligation(doc)
      due = document_due(doc)
      settled_on = doc.settled_at&.to_date

      if settled_on
        return nil if settled_on < @today - @lookback

        status = :settled
      elsif due.nil?
        return nil
      else
        status = due < @today ? :late : :due
      end
      return nil if due.nil? && status != :settled

      rec = recurrence[Money::Recurrence.group_key(doc)]
      Obligation.new(
        id: "doc:#{doc.id}",
        direction: doc.direction,
        counterpart: doc.entity_display_name,
        what: document_what(doc, rec),
        amount: ::Money.new(doc.amount_cents, doc.currency),
        due_on: due || settled_on,
        status: status,
        settled_on: settled_on,
        settled_via: (settled_status_via(doc) if status == :settled),
        source_email_message: doc.email_message,
        document: doc,
        reminder: nil,
        recurring: rec&.recurring? || false,
        cadence: rec&.cadence,
        next_renewal_on: rec&.next_renewal_on,
        due_estimated: @estimated_due&.include?(doc.id) || false,
        pay_url: pay_url_for(doc),
        actions: document_actions(doc.direction, status, pay_url_for(doc))
      )
    end

    # A due date, or — for an invoice with none — an estimate 30 days after its date,
    # flagged so the row can say "est.".
    def document_due(doc)
      date = safe_date(doc.due_date)
      return date if date

      if doc.document_type.in?(%w[expense_invoice revenue_invoice]) && (issued = safe_date(doc.document_date))
        (@estimated_due ||= Set.new) << doc.id
        return issued + 30.days
      end

      nil
    end

    def document_what(doc, rec)
      base =
        if doc.invoice_number.present? && doc.direction == :receivable
          I18n.t("money.what.invoice_sent", number: doc.invoice_number)
        elsif doc.invoice_number.present?
          I18n.t("money.what.invoice", number: doc.invoice_number)
        else
          doc.display_title
        end

      rec&.recurring? ? I18n.t("money.what.recurring", base: base) : base
    end

    def settled_status_via(doc)
      return nil unless doc.settled_bank_match?

      match = TransactionMatch.confirmed.where(document_id: doc.id)
                              .joins(:bank_transaction)
                              .order("bank_transactions.booked_on DESC").first
      return nil unless match

      txn = match.bank_transaction
      bank = txn.reconciliation&.bank_name.presence
      return nil unless bank || txn.position

      parts = [ bank, (I18n.t("money.settled.line", number: txn.position) if txn.position) ].compact
      parts.join(" · ")
    end

    def document_actions(direction, status, pay_url)
      return [] if status == :settled

      if direction == :receivable
        return status == :late ? %i[mark_paid send_reminder] : %i[remind_on]
      end

      # payable
      actions = [ :mark_paid ]
      actions << :pay if pay_url.present?
      actions
    end

    # metadata["payment_url"], or the first URL in the source email whose text/URL
    # reads like a payment link — never fabricated.
    def pay_url_for(doc)
      from_meta = doc.metadata.is_a?(Hash) ? doc.metadata["payment_url"].presence : nil
      return from_meta if from_meta

      pay_url_from_email(doc.email_message)
    end

    def pay_url_from_email(email)
      body = email&.try(:body_plain).presence || email&.try(:snippet).presence || email&.try(:body).presence
      return nil if body.blank?

      body.to_s.scan(%r{https?://[^\s"'<>)]+}).find { |url| url.match?(/pay|payment|checkout|invoice/i) }
    end

    # ── Reminders ────────────────────────────────────────────────────────────
    def reminder_obligations
      @reminder_obligations ||= candidate_reminders.filter_map { |rem| build_reminder_obligation(rem) }
    end

    def candidate_reminders
      window = (@today - @lookback)..(@today + @horizon)
      @workspace.reminders.accessible_to(@user)
                .where(reminder_type: %i[payment_due renewal], status: %i[pending confirmed])
                .where.not(amount_cents: nil)
                .select { |rem| rem.due_at && window.cover?(rem.due_at.to_date) }
                .reject { |rem| duplicate_of_document?(rem) }
    end

    # Skip a reminder whose obligation a document already covers — same source
    # document, or the same email thread that produced a document obligation.
    def duplicate_of_document?(rem)
      case rem.source_type
      when "Document"
        document_obligation_ids.include?(rem.source_id)
      when "EmailMessage"
        thread = rem.source&.email_thread_id
        thread.present? && document_thread_ids.include?(thread)
      else
        false
      end
    end

    def document_obligation_ids
      @document_obligation_ids ||= document_obligations.filter_map { |o| o.document&.id }.to_set
    end

    def document_thread_ids
      @document_thread_ids ||= document_obligations.filter_map { |o| o.source_email_message&.email_thread_id }.to_set
    end

    def build_reminder_obligation(rem)
      due = rem.due_at.to_date
      source_doc = rem.source if rem.source_type == "Document"

      if rem.renewal?
        direction = :payable
        status = :decide
        what = I18n.t("money.what.renewal", period: renewal_period(rem))
        actions = %i[keep cancel]
      else
        direction = reminder_direction(rem)
        status = due < @today ? :late : :due
        what = reminder_what(rem)
        actions = reminder_actions(direction, status)
      end

      Obligation.new(
        id: "rem:#{rem.id}",
        direction: direction,
        counterpart: reminder_counterpart(rem, source_doc),
        what: what,
        amount: rem.money,
        due_on: due,
        status: status,
        settled_on: nil,
        settled_via: nil,
        source_email_message: reminder_email(rem, source_doc),
        document: source_doc,
        reminder: rem,
        recurring: rem.renewal?,
        cadence: (:yearly if rem.renewal?),
        next_renewal_on: (due if rem.renewal?),
        due_estimated: false,
        pay_url: (pay_url_for(source_doc) if source_doc),
        actions: actions
      )
    end

    def reminder_direction(rem)
      email = rem.source if rem.source_type == "EmailMessage"
      email&.try(:sent?) ? :receivable : :payable
    end

    def reminder_counterpart(rem, source_doc)
      data = rem.extracted_data.is_a?(Hash) ? rem.extracted_data : {}
      data["counterpart"].presence || source_doc&.entity_display_name.presence || rem.title
    end

    def reminder_email(rem, source_doc)
      return rem.source if rem.source_type == "EmailMessage"

      source_doc&.email_message
    end

    def reminder_what(rem)
      rem.title.presence || I18n.t("money.what.payment_due")
    end

    def renewal_period(rem)
      data = rem.extracted_data.is_a?(Hash) ? rem.extracted_data : {}
      key = data["cadence"].presence || data["period"].presence || "yearly"
      I18n.t("money.cadence.#{key}", default: I18n.t("money.cadence.yearly"))
    end

    def reminder_actions(direction, status)
      if direction == :receivable
        status == :late ? %i[send_reminder] : %i[remind_on]
      else
        []
      end
    end

    def safe_date(value)
      value.respond_to?(:to_date) ? value.to_date : nil
    rescue StandardError
      nil
    end
  end
end
