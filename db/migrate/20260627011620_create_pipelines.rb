class CreatePipelines < ActiveRecord::Migration[8.1]
  def change
    create_table :pipelines, id: :uuid do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.string :name, null: false
      t.text :description
      t.string :icon, default: "git-branch", null: false
      t.integer :applies_to, default: 0, null: false
      t.integer :position, default: 0, null: false
      t.timestamps
    end
    add_index :pipelines, [:workspace_id, :name], unique: true
    add_index :pipelines, [:workspace_id, :position]
  end
end
