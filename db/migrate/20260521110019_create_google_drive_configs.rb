class CreateGoogleDriveConfigs < ActiveRecord::Migration[8.1]
  def change
    create_table :google_drive_configs do |t|
      t.references :document_type, null: false, foreign_key: true, index: { unique: true }
      t.boolean :auto_push, default: false, null: false
      t.string :folder_id
      t.string :folder_path
      t.string :naming_pattern, default: "{date}_{entity}_{reference}", null: false
      t.integer :subfolder_pattern, default: 0, null: false
      t.timestamps
    end
  end
end
