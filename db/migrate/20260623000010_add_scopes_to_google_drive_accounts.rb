class AddScopesToGoogleDriveAccounts < ActiveRecord::Migration[8.1]
  def change
    # Stores the space-separated OAuth scope string Google granted at connect time,
    # so we can detect accounts still on the legacy `drive.file` scope and prompt a
    # reconnect to unlock full folder browsing.
    add_column :google_drive_accounts, :scopes, :string
  end
end
