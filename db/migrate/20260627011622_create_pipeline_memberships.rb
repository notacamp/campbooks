class CreatePipelineMemberships < ActiveRecord::Migration[8.1]
  def change
    create_table :pipeline_memberships, id: :uuid, if_not_exists: true do |t|
      t.references :pipeline, null: false, foreign_key: true, type: :uuid
      t.references :item, polymorphic: true, null: false
      t.references :current_stage, foreign_key: { to_table: :pipeline_stages }, type: :uuid
      t.integer :position, default: 0, null: false
      t.datetime :entered_at
      t.datetime :last_moved_at
      t.jsonb :stage_history, default: [], null: false
      t.timestamps
    end

    add_index :pipeline_memberships, [:pipeline_id, :item_type, :item_id],
              unique: true, name: "idx_plm_on_pipeline_and_item", if_not_exists: true
    add_index :pipeline_memberships, [:item_type, :item_id],
              name: "idx_plm_on_item", if_not_exists: true
    add_index :pipeline_memberships, :current_stage_id, if_not_exists: true
  end
end
