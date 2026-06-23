class RenameEmailMessageColumns < ActiveRecord::Migration[8.1]
  def change
    # Remove index before renaming columns
    remove_index :email_messages, column: [ :email_account_id, :zoho_message_id ], unique: true

    rename_column :email_messages, :zoho_message_id, :provider_message_id
    rename_column :email_messages, :zoho_folder_id, :provider_folder_id

    add_index :email_messages, [ :email_account_id, :provider_message_id ], unique: true,
              name: :index_emails_on_account_and_provider_message
  end
end
