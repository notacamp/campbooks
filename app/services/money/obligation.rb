# frozen_string_literal: true

# NOTE: this file intentionally reopens the money-rails `Money` class to hang the
# app's Money-surface services under a `Money::` namespace (Money::Ledger,
# Money::Obligation, …). Zeitwerk resolves `app/services/money/*.rb` as constants
# nested in the already-defined `Money` class; `Money.new(cents, currency)` keeps
# working unchanged.
class Money
  # A single obligation on the Money surface: something owed, one way or the other,
  # that Scout read from an invoice, a bill or a renewal notice. It is a value
  # object (NOT an ActiveRecord table) — the ledger assembles it from a Document or
  # a Reminder, so the existing accounting/reminder substrate stays the source of
  # truth and settlement still comes from the bank.
  #
  #   direction : :receivable (owed TO you) | :payable (you owe)
  #   status    : :late | :due | :settled | :decide
  #   amount    : a money-rails Money (the obligation's own currency)
  #   actions   : the row/card affordances, as symbols, decided by the ledger
  #               (:mark_paid, :send_reminder, :pay, :remind_on, :keep, :cancel)
  Obligation = Struct.new(
    :id, :direction, :counterpart, :what, :amount, :due_on, :status,
    :settled_on, :settled_via, :source_email_message, :document, :reminder,
    :recurring, :cadence, :next_renewal_on, :due_estimated, :pay_url, :actions,
    # Attention enrichment (set by Money::Ledger#enrich_obligations!):
    :priority, :why, :counterpart_weight, :amount_ratio, :usual_delay_days, :attention_reason,
    keyword_init: true
  ) do
    def receivable? = direction == :receivable
    def payable?    = direction == :payable
    def late?       = status == :late
    def due?        = status == :due
    def settled?    = status == :settled
    def decide?     = status == :decide
    def recurring?  = recurring == true
    def due_estimated? = due_estimated == true

    def amount_cents = amount&.cents
    def currency     = amount&.currency&.iso_code || "EUR"

    # Signed cents for sorting/summing by direction (receivable positive, payable
    # negative). The amount itself is always stored as a positive Money.
    def signed_cents
      return 0 if amount_cents.nil?

      payable? ? -amount_cents : amount_cents
    end

    # Days past due (0 when not late). Positive integer.
    def days_late(today = Date.current)
      return 0 unless due_on && due_on < today

      (today - due_on).to_i
    end

    # DOM anchor for the timeline bars to link into the ledger row.
    def dom_id = "ob-#{id}"

    def action?(name) = Array(actions).include?(name)
  end
end
