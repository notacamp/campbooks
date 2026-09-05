# frozen_string_literal: true

module Reconciliations
  # Query object: browse all unlinked money-type documents in the reconciliation
  # window, sorted by closest amount to the bank line.
  #
  # "Unlinked" means the document has no confirmed TransactionMatch anywhere
  # in this reconciliation, so the user won't see an already-matched invoice.
  # (Documents matched in OTHER reconciliations are still shown — the user can
  # choose to attach them and the system will note the cross-recon overlap.)
  #
  # The date window is the reconciliation's period (period_start..period_end),
  # widened by 15 days on each side to capture invoices that are filed a few
  # days before or after the statement period.
  #
  # Usage:
  #   Reconciliations::SkimDocuments.new(
  #     bank_transaction: txn,
  #     reconciliation:   recon,
  #     workspace:        ws
  #   ).call
  #   # → [Document, ...] sorted by |doc.amount_cents - txn.amount_cents.abs|
  class SkimDocuments
    MAX_RESULTS    = 50
    WINDOW_PADDING = 15.days  # days to widen the period on each side

    def initialize(bank_transaction:, reconciliation:, workspace:)
      @txn            = bank_transaction
      @reconciliation = reconciliation
      @workspace      = workspace
    end

    # Returns Array<Document> sorted by closest absolute-amount match to the
    # bank line. Documents already confirmed in this reconciliation are excluded.
    def call
      target_abs = @txn.amount_cents.abs

      base_scope.to_a.sort_by do |doc|
        (doc.amount_cents.to_i.abs - target_abs).abs
      end
    end

    # The date window used for the query — exposed for the UI label.
    def window
      @window ||= begin
        raw_start = @reconciliation.period_start
        raw_end   = @reconciliation.period_end
        fallback  = @txn.booked_on

        start_date = (raw_start || fallback) - WINDOW_PADDING
        end_date   = (raw_end   || fallback) + WINDOW_PADDING
        (start_date..end_date)
      end
    end

    private

    def base_scope
      # Exclude documents that are already confirmed in this reconciliation
      # (attached to any bank transaction, not just this one).
      confirmed_doc_ids = TransactionMatch
        .joins(:bank_transaction)
        .where(status: :confirmed,
               bank_transactions: { reconciliation_id: @reconciliation.id })
        .pluck(:document_id)

      w = window

      @workspace.documents
                .money_types
                .where(
                  "documents.metadata->>'amount_cents' IS NOT NULL AND " \
                  "(CASE WHEN documents.metadata->>'amount_cents' ~ ? " \
                  "THEN (documents.metadata->>'amount_cents')::bigint END) <> 0",
                  Document::AMOUNT_CENTS_REGEX
                )
                .where.not(id: confirmed_doc_ids)
                .where(
                  "(documents.metadata->>'document_date' >= :start " \
                  "AND documents.metadata->>'document_date' <= :end) " \
                  "OR (documents.metadata->>'due_date' >= :start " \
                  "AND documents.metadata->>'due_date' <= :end)",
                  start: w.begin.to_s, end: w.end.to_s
                )
                .limit(MAX_RESULTS)
    end
  end
end
