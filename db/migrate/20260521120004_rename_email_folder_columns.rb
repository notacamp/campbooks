class RenameEmailFolderColumns < ActiveRecord::Migration[8.1]
  def change
    remove_index :email_folders, column: [ :email_account_id, :zoho_folder_id ], unique: true

    rename_column :email_folders, :zoho_folder_id, :provider_folder_id

    add_index :email_folders, [ :email_account_id, :provider_folder_id ], unique: true,
              name: :index_email_folders_on_email_account_id_and_provider_folder_id
  end
end
