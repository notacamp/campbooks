class AddNameToEmailAccounts < ActiveRecord::Migration[8.1]
  def change
    add_column :email_accounts, :name, :string
  end
end
