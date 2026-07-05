# frozen_string_literal: true

class CreateScheduledDigests < ActiveRecord::Migration[8.1]
  def change
    create_table :scheduled_digests, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.references :user, null: false, foreign_key: true, type: :uuid
      t.string :name, null: false
      t.string :preset_key
      t.jsonb :config, null: false, default: {}
      t.text :ai_instructions
      t.boolean :ai_enabled, null: false, default: true
      t.boolean :deliver_by_email, null: false, default: true
      t.boolean :show_in_feed, null: false, default: true
      t.boolean :enabled, null: false, default: true
      t.string :rrule, null: false
      t.datetime :next_run_at, null: false
      t.datetime :last_run_at
      t.timestamps
    end

    add_index :scheduled_digests, :next_run_at,
              where: "enabled",
              name: "index_scheduled_digests_on_next_run_at_enabled"

    create_table :digest_issues, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :scheduled_digest, null: false, foreign_key: true, type: :uuid
      t.uuid :workspace_id, null: false
      t.uuid :user_id, null: false
      t.integer :status, null: false, default: 0
      t.datetime :period_start, null: false
      t.datetime :period_end, null: false
      t.jsonb :content, null: false, default: {}
      t.boolean :ai_used, null: false, default: false
      t.string :error_message
      t.datetime :email_sent_at
      t.timestamps
    end

    add_index :digest_issues, [ :scheduled_digest_id, :period_end ],
              unique: true,
              name: "index_digest_issues_on_digest_and_period_end"
    add_index :digest_issues, [ :user_id, :created_at ],
              name: "index_digest_issues_on_user_and_created_at"
  end
end
