class BackfillReminderExtractionAiConfig < ActiveRecord::Migration[8.1]
  # Give existing workspaces a dedicated `reminder_extraction` AI config by mirroring
  # their `email_analysis` text config, so it shows up in the Settings → AI matrix and
  # the extractor has a dedicated knob. New workspaces get it from seeds/ProviderSetup;
  # Ai::ReminderExtractor also falls back to email_analysis at runtime, so this is a
  # convenience, not load-bearing. Raw SQL (no model coupling), idempotent via NOT EXISTS.
  def up
    execute(<<~SQL)
      INSERT INTO ai_configurations
        (workspace_id, purpose, ai_adapter_id, model, max_tokens, temperature, enabled, created_at, updated_at)
      SELECT src.workspace_id, 'reminder_extraction', src.ai_adapter_id, src.model,
             src.max_tokens, src.temperature, src.enabled, NOW(), NOW()
      FROM ai_configurations src
      WHERE src.purpose = 'email_analysis'
        AND NOT EXISTS (
          SELECT 1 FROM ai_configurations existing
          WHERE existing.workspace_id = src.workspace_id
            AND existing.purpose = 'reminder_extraction'
        )
    SQL
  end

  def down
    execute("DELETE FROM ai_configurations WHERE purpose = 'reminder_extraction'")
  end
end
