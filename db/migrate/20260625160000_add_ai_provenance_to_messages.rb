class AddAiProvenanceToMessages < ActiveRecord::Migration[8.1]
  def change
    # Records which AI provider/model/region produced an AI output, so the app can
    # show "Processed by <provider> · <region>" in context. Empty ({}) when the row
    # wasn't AI-generated. Constant default => PG11+ metadata-only backfill.
    add_column :agent_messages, :ai_provenance, :jsonb, default: {}, null: false
    add_column :email_messages, :ai_provenance, :jsonb, default: {}, null: false
  end
end
