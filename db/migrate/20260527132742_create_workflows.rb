class CreateWorkflows < ActiveRecord::Migration[8.1]
  def change
    create_table :workflows do |t|
      t.references :workspace, null: false, foreign_key: true
      t.string :name, null: false
      t.text :description
      t.boolean :enabled, default: true, null: false
      t.string :trigger_type, null: false
      t.jsonb :trigger_config, default: {}
      t.timestamps
    end

    create_table :workflow_steps do |t|
      t.references :workflow, null: false, foreign_key: true
      t.integer :position, null: false
      t.string :step_type, null: false
      t.string :action_type
      t.jsonb :config, default: {}
      t.timestamps
    end

    add_index :workflow_steps, [ :workflow_id, :position ]

    create_table :workflow_executions do |t|
      t.references :workflow, null: false, foreign_key: true
      t.references :workspace, null: false, foreign_key: true
      t.integer :status, default: 0, null: false
      t.jsonb :trigger_data, default: {}
      t.text :error_message
      t.datetime :started_at
      t.datetime :completed_at
      t.timestamps
    end

    create_table :workflow_execution_steps do |t|
      t.references :workflow_execution, null: false, foreign_key: true
      t.references :workflow_step, null: false, foreign_key: true
      t.integer :status, default: 0, null: false
      t.jsonb :input_data, default: {}
      t.jsonb :output_data, default: {}
      t.text :error_message
      t.datetime :started_at
      t.datetime :completed_at
      t.timestamps
    end
  end
end
