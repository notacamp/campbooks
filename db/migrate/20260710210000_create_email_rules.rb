# frozen_string_literal: true

# Workspace-scoped, user-defined deterministic rules evaluated against every
# newly ingested email.  When an email matches the rule's criteria the rule
# applies its actions (tag, archive, mark read, move to folder).  Criteria are
# stored as jsonb; actions live as plain boolean/FK columns.
class CreateEmailRules < ActiveRecord::Migration[8.1]
  def change
    create_table :email_rules, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :workspace,    null: false, foreign_key: true, type: :uuid
      t.references :created_by,   null: true,
                   foreign_key: { to_table: :users, on_delete: :nullify }, type: :uuid
      t.references :mail_folder,  null: true,
                   foreign_key: { to_table: :mail_folders, on_delete: :nullify }, type: :uuid

      t.string  :name,          null: false
      t.jsonb   :criteria,      null: false, default: {}
      t.boolean :archive,       null: false, default: false
      t.boolean :mark_read,     null: false, default: false
      t.boolean :enabled,       null: false, default: true
      t.bigint  :matched_count, null: false, default: 0
      t.datetime :last_run_at

      t.timestamps
    end

    add_index :email_rules, :workspace_id,
              name: "index_email_rules_on_workspace_id_and_enabled",
              where: "enabled = TRUE"
  end
end
