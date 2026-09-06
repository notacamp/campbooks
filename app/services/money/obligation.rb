# frozen_string_literal: true

# NOTE: this file intentionally reopens the money-rails `Money` class to hang the
# app's Money-surface services under a `Money::` namespace. `Money.new(cents, currency)`
# keeps working unchanged.
class Money
  # A single item on the Money surface derived from a money document.
  # It is a value object (NOT an ActiveRecord table) assembled by Money::Ledger.
  #
  #   direction  : :receivable (owed TO you) | :payable (you owe)
  #   status     : :settled | :missing | :unconfirmed
  #   amount     : a money-rails Money object
  #   actions    : the row affordances as symbols, decided by the ledger
  Obligation = Struct.new(
    :id, :direction, :counterpart, :what, :amount, :anchor_on, :status,
    :settled_on, :settled_via, :source_email_message, :document,
    :statement, :statement_label, :actions,
    keyword_init: true
  ) do
    def receivable?  = direction == :receivable
    def payable?     = direction == :payable
    def settled?     = status == :settled
    def missing?     = status == :missing
    def unconfirmed? = status == :unconfirmed

    def amount_cents = amount&.cents
    def currency     = amount&.currency&.iso_code || "EUR"

    def dom_id = "ob-#{id}"

    def action?(name) = Array(actions).include?(name)
  end
end
