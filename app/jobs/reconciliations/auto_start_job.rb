# frozen_string_literal: true

module Reconciliations
  # Turns a filed bank statement into a reconciliation in the background (see
  # Reconciliations::AutoStart). Enqueued by DocumentProcessJob when Scout files a
  # document as a bank statement, and by Money's "Reconcile them" for the backlog.
  class AutoStartJob < ApplicationJob
    queue_as :default
    discard_on ActiveRecord::RecordNotFound

    def perform(document_id, created_by_id: nil)
      document = Document.find(document_id)
      Current.workspace = document.workspace
      created_by = User.find_by(id: created_by_id) if created_by_id

      result = Reconciliations::AutoStart.call(document, created_by: created_by)
      Rails.logger.info("[Reconciliations::AutoStartJob] document #{document_id}: #{result.started? ? 'started' : result.reason}")
    ensure
      Current.workspace = nil
    end
  end
end
