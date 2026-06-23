class CreateNotionPages < ActiveRecord::Migration[8.1]
  def change
    create_table :notion_pages do |t|
      t.references :document, null: false, foreign_key: true, index: { unique: true }
      t.references :notion_database_mapping, null: false, foreign_key: true
      t.string :notion_page_id, null: false
      t.datetime :last_synced_at
      t.integer :sync_status, default: 0, null: false
      t.text :last_error

      t.timestamps
    end

    add_index :notion_pages, :notion_page_id, unique: true
    add_index :notion_pages, :sync_status
  end
end
