class AddOrganizationContextToUsers < ActiveRecord::Migration[8.1]
  def change
    add_column :users, :organization_context, :text
  end
end
