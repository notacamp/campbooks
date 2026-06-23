class CreateGoogleDriveAccounts < ActiveRecord::Migration[8.1]
  def change
    create_table :google_drive_accounts do |t|
      t.string :email
      t.string :refresh_token, null: false
      t.boolean :connected, default: true, null: false
      t.timestamps
    end
  end
end
