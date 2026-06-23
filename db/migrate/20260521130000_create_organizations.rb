class CreateOrganizations < ActiveRecord::Migration[8.1]
  def change
    create_table :organizations do |t|
      t.string :name, null: false
      t.string :slug, null: false
      t.jsonb :settings, default: {}, null: false
      t.timestamps
      t.index :slug, unique: true
    end

    # Add organization_id to tenant-scoped tables (nullable, no FK yet)
    add_column :users, :organization_id, :bigint
    add_column :email_accounts, :organization_id, :bigint
    add_column :documents, :organization_id, :bigint
    add_column :document_types, :organization_id, :bigint
    add_column :tags, :organization_id, :bigint
    add_column :ai_configurations, :organization_id, :bigint
    add_column :notion_integrations, :organization_id, :bigint
    add_column :contacts, :organization_id, :bigint
    add_column :people, :organization_id, :bigint
    add_column :agent_threads, :organization_id, :bigint
    add_column :monthly_reports, :organization_id, :bigint
    add_column :google_drive_accounts, :organization_id, :bigint

    reversible do |dir|
      dir.up do
        # Store existing organization_context before removing
        ctx_row = execute("SELECT organization_context FROM users WHERE organization_context IS NOT NULL AND organization_context != '' LIMIT 1").first
        existing_context = ctx_row&.dig("organization_context")

        org_settings = if existing_context
          ActiveRecord::Base.connection.quote({ organization_context: existing_context }.to_json)
        else
          "'{}'"
        end

        say "Creating default organization and backfilling records..."
        result = execute("INSERT INTO organizations (name, slug, settings, created_at, updated_at) VALUES ('Default', 'default', #{org_settings}, NOW(), NOW()) RETURNING id")
        default_org_id = result.first["id"]

        %w[users email_accounts documents document_types tags ai_configurations notion_integrations contacts people agent_threads monthly_reports google_drive_accounts].each do |table|
          execute("UPDATE #{table} SET organization_id = #{default_org_id} WHERE organization_id IS NULL")
        end
      end
    end

    remove_column :users, :organization_context, :text

    # Add indexes and foreign keys
    add_index :users, :organization_id
    add_index :email_accounts, :organization_id
    add_index :documents, :organization_id
    add_index :document_types, :organization_id
    add_index :tags, :organization_id
    add_index :ai_configurations, :organization_id
    add_index :notion_integrations, :organization_id
    add_index :contacts, :organization_id
    add_index :people, :organization_id
    add_index :agent_threads, :organization_id
    add_index :monthly_reports, :organization_id
    add_index :google_drive_accounts, :organization_id

    add_foreign_key :users, :organizations
    add_foreign_key :email_accounts, :organizations
    add_foreign_key :documents, :organizations
    add_foreign_key :document_types, :organizations
    add_foreign_key :tags, :organizations
    add_foreign_key :ai_configurations, :organizations
    add_foreign_key :notion_integrations, :organizations
    add_foreign_key :contacts, :organizations
    add_foreign_key :people, :organizations
    add_foreign_key :agent_threads, :organizations
    add_foreign_key :monthly_reports, :organizations
    add_foreign_key :google_drive_accounts, :organizations
  end
end
