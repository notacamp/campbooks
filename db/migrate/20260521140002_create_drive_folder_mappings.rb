class CreateDriveFolderMappings < ActiveRecord::Migration[8.1]
  def change
    create_table :drive_folder_mappings do |t|
      t.references :zoho_drive_account, null: false, foreign_key: true
      t.references :document_type, null: true, foreign_key: true
      t.string :drive_folder_id, null: false
      t.string :drive_folder_path
      t.boolean :auto_sync, default: false, null: false
      t.timestamps
    end

    add_index :drive_folder_mappings,
              [ :zoho_drive_account_id, :document_type_id ],
              unique: true,
              name: "idx_drive_folder_mappings_on_account_and_type"
  end
end
