class RenameEmailAccountColumns < ActiveRecord::Migration[8.1]
  def change
    rename_column :email_accounts, :zoho_account_id, :provider_account_id
    rename_column :email_accounts, :zoho_refresh_token, :refresh_token
  end
end
