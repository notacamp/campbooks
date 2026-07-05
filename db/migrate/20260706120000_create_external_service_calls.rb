# frozen_string_literal: true

class CreateExternalServiceCalls < ActiveRecord::Migration[8.1]
  def change
    create_table :external_service_calls, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.string :service, null: false
      t.string :operation
      t.integer :status, null: false, default: 0
      t.integer :http_status
      t.integer :duration_ms
      t.string :error_class
      t.text :error_message
      t.uuid :workspace_id            # denormalized, deliberately no FK (log rows may outlive the workspace)
      t.jsonb :metadata, null: false, default: {}
      t.datetime :created_at, null: false   # immutable rows — no updated_at
    end

    add_index :external_service_calls, [ :service, :created_at ],
      name: "index_external_service_calls_on_service_and_created_at"
    add_index :external_service_calls, :created_at,
      name: "index_external_service_calls_on_created_at"
    add_index :external_service_calls, [ :status, :created_at ],
      name: "index_external_service_calls_on_error_and_created_at",
      where: "status = 1"
  end
end
