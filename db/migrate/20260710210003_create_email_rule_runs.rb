# frozen_string_literal: true

# Bookkeeping for a retroactive rule run — a user-triggered sweep of the
# existing inbox against one rule.  Tracks progress, undo data, and final
# counts.  Undo data is only stored when matched_count <= 25_000 (undoable:
# true); above that the arrays stay empty and the run is marked not undoable.
class CreateEmailRuleRuns < ActiveRecord::Migration[8.1]
  def change
    create_table :email_rule_runs, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :email_rule, null: false,
                   foreign_key: { on_delete: :cascade }, type: :uuid
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.references :started_by, null: true,
                   foreign_key: { to_table: :users, on_delete: :nullify }, type: :uuid

      t.integer :status,          null: false, default: 0
      t.integer :matched_count,   null: false, default: 0
      t.integer :processed_count, null: false, default: 0

      # Undo bookkeeping: only populated when undoable is true.
      t.jsonb :tagged_email_ids,     null: false, default: []
      t.jsonb :archived_email_ids,   null: false, default: []
      t.jsonb :marked_read_email_ids, null: false, default: []
      t.jsonb :moved_email_ids,      null: false, default: []
      t.boolean :undoable,           null: false, default: true

      t.datetime :finished_at

      t.timestamps
    end
  end
end
