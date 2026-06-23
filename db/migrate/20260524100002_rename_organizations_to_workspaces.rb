class RenameOrganizationsToWorkspaces < ActiveRecord::Migration[8.1]
  def change
    # Notion integrations has its own workspace_id/workspace_name (Notion API concepts).
    # Rename those first to free up the column names for the tenant FK.
    rename_column :notion_integrations, :workspace_id, :notion_workspace_id
    rename_column :notion_integrations, :workspace_name, :notion_workspace_name

    # Rename the table
    rename_table :organizations, :workspaces

    # Rename all organization_id FK columns
    rename_column :agent_threads, :organization_id, :workspace_id
    rename_column :ai_adapters, :organization_id, :workspace_id
    rename_column :ai_configurations, :organization_id, :workspace_id
    rename_column :contacts, :organization_id, :workspace_id
    rename_column :document_types, :organization_id, :workspace_id
    rename_column :documents, :organization_id, :workspace_id
    rename_column :email_accounts, :organization_id, :workspace_id
    rename_column :exports, :organization_id, :workspace_id
    rename_column :google_drive_accounts, :organization_id, :workspace_id
    rename_column :invitations, :organization_id, :workspace_id
    rename_column :notion_integrations, :organization_id, :workspace_id
    rename_column :people, :organization_id, :workspace_id
    rename_column :search_chunks, :organization_id, :workspace_id
    rename_column :search_records, :organization_id, :workspace_id
    rename_column :search_tag_embeddings, :organization_id, :workspace_id
    rename_column :tags, :organization_id, :workspace_id
    rename_column :users, :organization_id, :workspace_id

    # Rename custom-named indexes that don't auto-rename with columns
    rename_index :ai_configurations, "index_ai_configurations_on_org_and_purpose",
                 "index_ai_configurations_on_workspace_and_purpose"
    rename_index :document_types, "index_document_types_on_org_and_name",
                 "index_document_types_on_workspace_and_name"
    rename_index :invitations, "idx_invitations_on_email_org_status",
                 "idx_invitations_on_email_workspace_status"
  end
end
