module Tasks
  # Best-effort task extraction from a processed document — a contract or proposal
  # may carry an action the reader owns (sign, review, return). Runs a cheap TEXT
  # pass over the already-extracted structured data (no second vision call). Gated
  # by the readiness flag and the :tasks entitlement. Enqueued from DocumentProcessJob.
  class DocumentExtractionJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    # Document types that never carry an action for the reader — skip the LLM call.
    SKIP_TYPES = %w[bank_statement receipt identification bank_journal_entry].freeze

    def perform(document_id)
      return unless Features.tasks?

      document = Document.find_by(id: document_id)
      return unless document&.ai_completed?
      return if SKIP_TYPES.include?(document.document_type)

      workspace = document.workspace
      return unless workspace.entitlements.feature?(:tasks)
      return unless Ai::ProviderSetup.configured?(workspace, :text)

      Current.workspace = workspace

      content = [
        document.description,
        document.ai_extraction_data.presence&.to_json,
        document.metadata.presence&.to_json
      ].compact_blank.join("\n")

      memory = task_learning_memory(workspace)
      known  = Commitments::Known.for(workspace: workspace, source: document)

      items = Ai::TaskExtractor.new(
        source:            document,
        content:           content,
        anchor_date:       document.document_date || document.created_at.to_date,
        time_zone:         Time.zone,
        workspace:         workspace,
        learning_memory:   memory,
        known_commitments: known
      ).extract

      tasks = Tasks::Builder.call(
        workspace: workspace, source: document, raw_items: items, anchor_tz: Time.zone, learning_memory: memory
      )

      Feed::RefreshJob.enqueue_for_workspace(workspace) if tasks.any?
    ensure
      Current.workspace = nil
    end

    private

    # One memory per run, shared by the extractor (soft prompt hint) and the builder
    # (deterministic suppression). Best-effort: a failure here just means no learning.
    def task_learning_memory(workspace)
      Learning::Memory.new(source: Learning::Sources::Tasks.new(workspace))
    rescue => e
      Rails.logger.warn("[#{self.class.name}] learning_memory failed: #{e.message}")
      nil
    end
  end
end
