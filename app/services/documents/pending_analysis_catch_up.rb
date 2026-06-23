# frozen_string_literal: true

module Documents
  # Opening Skim catches up any document that was ingested but never AI-analyzed
  # (e.g. uploaded while the worker or AI provider was down): those sit ai_pending
  # and never reach the review queue (SkimScope only surfaces ai_completed docs).
  # Enqueuing DocumentProcessJob gets them analyzed so they join the queue.
  #
  # Safe + idempotent: DocumentProcessJob self-guards on ai_pending? (so a doc that
  # is already processing/completed is a no-op) and on the provider being configured.
  # We also gate here so a workspace with no provider doesn't enqueue a storm of
  # no-op jobs, and cap each pass so a pathological backlog can't flood the queue —
  # the remainder is picked up on the next Skim open.
  class PendingAnalysisCatchUp
    LIMIT = 100

    def self.run(workspace)
      return unless workspace
      return unless Ai::ProviderSetup.configured?(workspace, :documents)

      workspace.documents
               .where(ai_status: :pending)
               .limit(LIMIT)
               .pluck(:id)
               .each { |id| DocumentProcessJob.perform_later(id) }
    end
  end
end
