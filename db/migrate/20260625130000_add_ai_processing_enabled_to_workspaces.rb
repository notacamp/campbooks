class AddAiProcessingEnabledToWorkspaces < ActiveRecord::Migration[8.1]
  def change
    # Master "process my data with AI" switch for the workspace — the global AI
    # kill-switch on Settings → Data & Privacy. Default true so existing
    # workspaces keep working; PG11+ applies the constant default as a
    # metadata-only change, so no separate backfill migration is needed.
    add_column :workspaces, :ai_processing_enabled, :boolean, default: true, null: false
  end
end
