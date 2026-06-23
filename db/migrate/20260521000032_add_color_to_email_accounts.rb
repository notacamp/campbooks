class AddColorToEmailAccounts < ActiveRecord::Migration[8.1]
  def change
    add_column :email_accounts, :color, :string, null: false, default: "#3b82f6"
  end
end
