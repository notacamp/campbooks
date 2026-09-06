# frozen_string_literal: true

module Reconciliations
  # Extracts the atomic mutations on BankTransactions so both
  # Reconciliations::BankTransactionsController and MoneyController call the same
  # behaviour. Status transitions and match handling are byte-identical to the
  # original controller code.
  class LineActions
    attr_reader :transaction

    def initialize(transaction)
      @transaction = transaction
    end

    # Confirm a suggested match. Raises ActiveRecord::RecordNotFound when the match
    # does not belong to the transaction.
    def confirm!(match_id)
      match = @transaction.transaction_matches.find(match_id)

      ActiveRecord::Base.transaction do
        match.update!(status: :confirmed)
        @transaction.update!(status: :matched)
      end

      @transaction.reload
    end

    # Reject a suggested match. Demotes the transaction to :suggested when other
    # suggestions remain, else :unmatched.
    def reject!(match_id)
      match = @transaction.transaction_matches.find(match_id)
      match.update!(status: :rejected)

      remaining = @transaction.transaction_matches.suggested
      new_status = remaining.any? ? :suggested : :unmatched
      @transaction.update!(status: new_status)

      @transaction.reload
    end

    # Exclude the transaction with a reason. Raises ArgumentError for invalid reasons.
    def exclude!(reason)
      unless Reconciliations::BankTransactionsController::VALID_EXCLUSION_REASONS.include?(reason.to_s)
        raise ArgumentError, "Invalid exclusion reason: #{reason}"
      end

      @transaction.update!(status: :excluded, exclusion_reason: reason.to_s)
      @transaction.reload
    end

    # Reset the transaction back to :unmatched (undo for confirm/exclude).
    def reset!
      ActiveRecord::Base.transaction do
        @transaction.transaction_matches.confirmed
                    .update_all(status: TransactionMatch.statuses[:rejected])
        @transaction.update!(
          status:           :unmatched,
          exclusion_reason: nil,
          requested_at:     nil,
          requested_by:     nil
        )

        if @transaction.transaction_matches.reload.suggested.any?
          @transaction.update!(status: :suggested)
        end
      end

      @transaction.reload
    end

    # Manually match the transaction to a document.
    def manual_match!(document)
      match = @transaction.transaction_matches.find_or_initialize_by(document_id: document.id)
      match.assign_attributes(
        status:        :confirmed,
        matched_by:    :manual,
        confidence:    1.0,
        match_reasons: { "manual" => true }
      )
      match.save!
      @transaction.update!(status: :matched)

      @transaction.reload
    end
  end
end
