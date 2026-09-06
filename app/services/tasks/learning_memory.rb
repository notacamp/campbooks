module Tasks
  # One task-learning memory for an extraction run, shared by the analyzer's ask
  # staging and the backfill/document extraction jobs. It feeds both the extractor's
  # soft prompt hint and the builder's deterministic suppression. Best-effort: any
  # failure here just means no learning signal (never poison the pipeline), so it
  # returns nil rather than raising.
  module LearningMemory
    module_function

    def for(workspace)
      Learning::Memory.new(source: Learning::Sources::Tasks.new(workspace))
    rescue => e
      Rails.logger.warn("[Tasks::LearningMemory] failed for workspace #{workspace&.id}: #{e.message}")
      nil
    end
  end
end
