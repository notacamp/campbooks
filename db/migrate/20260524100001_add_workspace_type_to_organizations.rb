class AddWorkspaceTypeToOrganizations < ActiveRecord::Migration[8.1]
  def change
    add_column :organizations, :workspace_type, :string, default: "company", null: false

    add_check_constraint :organizations,
                         "workspace_type IN ('company', 'individual')",
                         name: "chk_organizations_workspace_type"
  end
end
