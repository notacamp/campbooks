class CreateNotionIntegrations < ActiveRecord::Migration[8.1]
  def change
    create_table :notion_integrations do |t|
      t.text :access_token, null: false
      t.string :workspace_name
      t.string :workspace_id
      t.boolean :active, default: true, null: false

      t.timestamps
    end
  end
end
