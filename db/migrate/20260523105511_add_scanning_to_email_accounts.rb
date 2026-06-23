class AddScanningToEmailAccounts < ActiveRecord::Migration[8.1]
  def change
    add_column :email_accounts, :scanning, :boolean, default: false, null: false
    add_column :email_accounts, :scan_started_at, :datetime
  end
end
