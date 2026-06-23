class CreateEmailFolders < ActiveRecord::Migration[8.1]
  def change
    create_table :email_folders do |t|
      t.references :email_account, null: false, foreign_key: true
      t.string :zoho_folder_id, null: false
      t.string :name, null: false
      t.integer :position, null: false, default: 0

      t.timestamps
    end

    add_index :email_folders, [ :email_account_id, :zoho_folder_id ], unique: true
  end
end
