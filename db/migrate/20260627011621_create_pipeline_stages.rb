class CreatePipelineStages < ActiveRecord::Migration[8.1]
  def change
    create_table :pipeline_stages, id: :uuid do |t|
      t.references :pipeline, null: false, foreign_key: true, type: :uuid
      t.string :name, null: false
      t.string :color, default: "#6366f1", null: false
      t.integer :position, null: false
      t.text :description
      t.boolean :is_terminal, default: false, null: false
      t.jsonb :auto_assign_rules, default: {}, null: false
      t.jsonb :exit_action_config, default: {}, null: false
      t.timestamps
    end
    add_index :pipeline_stages, [:pipeline_id, :name], unique: true
    add_index :pipeline_stages, [:pipeline_id, :position]
  end
end
