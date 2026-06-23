class FixAiConfigurationsUniqueIndex < ActiveRecord::Migration[8.1]
  def change
    remove_index :ai_configurations, name: "index_ai_configurations_on_purpose"
    add_index :ai_configurations, [ :organization_id, :purpose ], unique: true,
              name: "index_ai_configurations_on_org_and_purpose"
  end
end
