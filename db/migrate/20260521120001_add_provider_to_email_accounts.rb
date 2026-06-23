class AddProviderToEmailAccounts < ActiveRecord::Migration[8.1]
  def change
    add_column :email_accounts, :provider, :integer, default: 0, null: false
    add_index :email_accounts, :provider
  end
end
