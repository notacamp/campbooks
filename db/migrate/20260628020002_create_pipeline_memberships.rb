class CreatePipelineMemberships < ActiveRecord::Migration[8.1]
  def change
    create_table :pipeline_memberships do |t|
      t.references :pipeline, null: false, foreign_key: true
      t.references :item, polymorphic: true, null: false, index: false
      t.references :current_stage, foreign_key: { to_table: :pipeline_stages, on_delete: :nullify }
      t.integer :position, default: 0, null: false
      t.datetime :entered_at
      t.datetime :last_moved_at
      t.jsonb :stage_history, default: [], null: false
      t.timestamps
    end

    add_index :pipeline_memberships, [ :pipeline_id, :item_type, :item_id ],
              unique: true, name: "idx_plm_on_pipeline_and_item"
    add_index :pipeline_memberships, [ :item_type, :item_id ], name: "idx_plm_on_item"
  end
end
