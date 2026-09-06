# frozen_string_literal: true

class Money
  # Builds the Money surface's obligations from money documents only.
  # No reminders, no recurrence, no pay-URL sniffing, no estimated due dates.
  #
  # Lateness requires evidence: a document is :missing only when a ready
  # statement covers its anchor date (with a 7-day grace) and shows no
  # confirmed match. Until then it is :unconfirmed and INVISIBLE on the surface.
  #
  #   ledger = Money::Ledger.for(workspace, user, today:, evidence:)
  #   ledger.missing          # obligations with status :missing
  #   ledger.settled          # settled within the last 45 days, newest first
  #   ledger.unconfirmed      # obligations with status :unconfirmed
  #   ledger.find("doc:<uuid>") # one obligation, for a row action
  class Ledger
    SETTLED_LOOKBACK = 45.days

    def self.for(workspace, user, today: Date.current, evidence: nil)
      new(workspace, user, today: today, evidence: evidence)
    end

    def initialize(workspace, user, today:, evidence: nil)
      @workspace = workspace
      @user      = user
      @today     = today
      @evidence  = evidence || Money::Evidence.for(workspace)
    end

    def obligations
      @obligations ||= build_obligations
    end

    def missing
      obligations.select(&:missing?)
    end

    def settled
      obligations.select(&:settled?)
                 .sort_by { |o| o.settled_on || o.anchor_on }.reverse
    end

    def unconfirmed
      obligations.select(&:unconfirmed?)
    end

    # Sections for the CSV export. Only includes non-empty sections.
    def sections
      [ [ :missing, missing ], [ :settled, settled ] ]
        .reject { |(_key, list)| list.empty? }
    end

    def any? = obligations.any?

    def find(id)
      obligations.find { |o| o.id == id }
    end

    # Hash of currency => cents for missing payable obligations.
    def missing_cents_by_currency
      missing.select(&:payable?)
             .each_with_object({}) do |o, acc|
               acc[o.currency] = (acc[o.currency] || 0) + o.amount_cents.to_i
             end
    end

    private

    def build_obligations
      docs = money_documents
      # Sort by anchor descending (newest first)
      docs.filter_map { |doc| build_obligation(doc) }
          .sort_by { |o| [ o.anchor_on || Date.new(1970, 1, 1), o.counterpart.to_s ] }
          .reverse
    end

    def money_documents
      @workspace.documents.accessible_to(@user)
                .money_types
                .includes(:email_message)
                .reject(&:review_rejected?)
                .select { |doc| doc.amount_cents.present? && doc.direction }
    end

    def build_obligation(doc)
      status = @evidence.status_for(doc)

      # Skip settled documents older than the lookback window.
      if status == :settled
        settled_on = doc.settled_at&.to_date
        return nil if settled_on && settled_on < @today - SETTLED_LOOKBACK
      end

      anchor = @evidence.anchor_for(doc)
      stmt   = @evidence.statement_for(doc)

      Obligation.new(
        id:                   "doc:#{doc.id}",
        direction:            doc.direction,
        counterpart:          doc.entity_display_name,
        what:                 document_what(doc),
        amount:               ::Money.new(doc.amount_cents, doc.currency),
        anchor_on:            anchor,
        status:               status,
        settled_on:           doc.settled_at&.to_date,
        settled_via:          settled_status_via(doc),
        source_email_message: doc.email_message,
        document:             doc,
        statement:            stmt,
        statement_label:      stmt ? @evidence.label_for(stmt) : nil,
        actions:              obligation_actions(doc.direction, status)
      )
    end

    def document_what(doc)
      if doc.invoice_number.present? && doc.direction == :receivable
        I18n.t("money.what.invoice_sent", number: doc.invoice_number)
      elsif doc.invoice_number.present?
        I18n.t("money.what.invoice", number: doc.invoice_number)
      else
        doc.display_title
      end
    end

    def obligation_actions(direction, status)
      return [] if status == :settled
      return [] if status == :unconfirmed

      # :missing
      if direction == :payable
        %i[paid_elsewhere mark_paid]
      else
        %i[send_reminder mark_paid]
      end
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
  end
end
