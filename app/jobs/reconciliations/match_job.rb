# frozen_string_literal: true

module Reconciliations
  # Runs the heuristic + AI matching engine over a Reconciliation's unmatched
  # bank transactions, then sets status :ready and broadcasts the final state.
  #
  # Error handling:
  #   TRANSIENT errors (rate-limit, network)     → log + re-raise only; status stays :matching;
  #                                                retry_on reschedules the attempt
  #   Anything else (bug, data error)            → status :failed, broadcast, re-raise
  class MatchJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3
    discard_on ActiveJob::DeserializationError

    def perform(reconciliation_id)
      @reconciliation = Reconciliation.find(reconciliation_id)
      Current.workspace = @reconciliation.workspace

      Reconciliations::Matcher.new(
        reconciliation: @reconciliation,
        workspace:      @reconciliation.workspace
      ).call

      @reconciliation.update!(status: :ready)
      broadcast_update!
      Notifier.reconciliation_ready(@reconciliation)

    rescue *Ai::Adapters::Base::TRANSIENT_ERRORS => e
      Rails.logger.warn("[Reconciliations::MatchJob] Transient error for #{reconciliation_id}: #{e.class}: #{e.message}")
      raise

    rescue StandardError => e
      @reconciliation&.update_columns(
        status:      Reconciliation.statuses[:failed],
        parse_error: "Matching: #{e.class}: #{e.message.first(400)}",
        updated_at:  Time.current
      )
      broadcast_update!
      raise

    ensure
      Current.workspace = nil
    end

    private

    def broadcast_update!
      locale = @reconciliation.created_by&.locale.presence || I18n.default_locale
      transaction_count = @reconciliation.bank_transactions.count

      html = I18n.with_locale(locale) do
        ApplicationController.render(
          partial: "reconciliations/show_content",
          locals:  {
            reconciliation: @reconciliation,
            transactions:   @reconciliation.bank_transactions.ordered
                                           .includes(transaction_matches: :document).limit(50),
            next_page:      (transaction_count > 50 ? 2 : nil)
          },
          layout: false
        )
      end

      Turbo::StreamsChannel.broadcast_update_to(
        "reconciliation_#{@reconciliation.id}",
        target: "reconciliation_content",
        html:   html
      )
    rescue => e
      Rails.logger.warn("[Reconciliations::MatchJob] broadcast failed: #{e.class}: #{e.message}")
    end
  end
end
