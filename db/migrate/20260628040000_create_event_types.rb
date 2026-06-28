class CreateEventTypes < ActiveRecord::Migration[8.1]
  def change
    create_table :event_types do |t|
      t.references :workspace, null: false, foreign_key: true
      t.string :name, null: false
      t.string :color, null: false

      t.timestamps
    end

    add_index :event_types, [ :workspace_id, :name ], unique: true,
              name: "index_event_types_on_workspace_and_name"
  end
end
