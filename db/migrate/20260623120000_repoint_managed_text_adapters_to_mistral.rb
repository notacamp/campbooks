class RepointManagedTextAdaptersToMistral < ActiveRecord::Migration[8.1]
  # One-off, idempotent data heal — runs on deploy via db:prepare so it can't be
  # forgotten. Mirror of Ai::ManagedTextRepointer as of this migration, but FROZEN:
  # a migration must not couple to app code (constants/services) that can change
  # underneath it. Re-running is a no-op (valid rows are skipped).
  #
  # Fixes the prod state where managed "Campbooks AI — Text" adapters were repointed
  # to Mistral but left with a previous provider's model (e.g. ws#2 email_classification
  # stranded on `deepseek-v4-pro`), which Mistral rejects with HTTP 400 — silently
  # killing AI email tagging. Resets such text-purpose models to the platform default.
  ADAPTER_NAME  = "Campbooks AI — Text".freeze
  TARGET        = "mistral".freeze
  DEFAULT_MODEL = "mistral-small-latest".freeze
  VALID_MODELS  = %w[mistral-large-latest mistral-medium-latest mistral-small-latest ministral-8b-latest].freeze
  TEXT_PURPOSES = %w[
    global_chat email_chat compose_chat email_analysis
    email_classification draft_reply reminder_extraction
  ].freeze

  class MigAdapter < ActiveRecord::Base
    self.table_name = "ai_adapters"
  end

  class MigConfig < ActiveRecord::Base
    self.table_name = "ai_configurations"
  end

  def up
    adapter_ids = MigAdapter.where(name: ADAPTER_NAME, managed: true).pluck(:id)
    return if adapter_ids.empty?

    repointed = MigAdapter.where(id: adapter_ids).where.not(provider: TARGET)
                          .update_all(provider: TARGET, updated_at: Time.current)

    fixed = MigConfig.where(ai_adapter_id: adapter_ids, purpose: TEXT_PURPOSES)
                     .where.not(model: VALID_MODELS)
                     .update_all(model: DEFAULT_MODEL, updated_at: Time.current)

    say "Repointed #{repointed} managed text adapter(s) to #{TARGET}; normalized #{fixed} stale model(s) to #{DEFAULT_MODEL}."
  end

  def down
    # No-op: prior per-config models aren't recoverable, and the heal isn't destructive.
  end
end
