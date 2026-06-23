class CreateDocumentDriveUploads < ActiveRecord::Migration[8.1]
  def change
    create_table :document_drive_uploads do |t|
      t.references :document, null: false, foreign_key: true
      t.references :zoho_drive_account, null: false, foreign_key: true
      t.string :drive_file_id
      t.string :status, null: false, default: "pending"
      t.text :error_message
      t.datetime :uploaded_at
      t.timestamps
    end

    add_index :document_drive_uploads, :status
  end
end
