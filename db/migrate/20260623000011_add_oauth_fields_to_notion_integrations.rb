class AddOauthFieldsToNotionIntegrations < ActiveRecord::Migration[8.1]
  def change
    add_column :notion_integrations, :notion_workspace_icon, :string
    add_column :notion_integrations, :bot_id, :string
    add_reference :notion_integrations, :authorized_by_user,
                  foreign_key: { to_table: :users }, null: true

    # A workspace can connect multiple Notion workspaces (one row each), but not the
    # same Notion workspace twice. Postgres treats NULLs as distinct, so legacy
    # manual-token rows (no notion_workspace_id) are unaffected.
    add_index :notion_integrations, [ :workspace_id, :notion_workspace_id ],
              unique: true, name: "index_notion_integrations_on_workspace_and_notion_ws"
  end
end
