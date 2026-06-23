class AddGoogleDriveFieldsToDocuments < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :google_drive_file_id, :string
    add_column :documents, :google_drive_push_status, :integer, default: 0, null: false
    add_column :documents, :google_drive_pushed_at, :datetime
    add_column :documents, :google_drive_push_error, :text
  end
end
