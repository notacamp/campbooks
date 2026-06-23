class AddOrganizationIdToGoogleDriveAccounts < ActiveRecord::Migration[8.1]
  def change
    add_reference :google_drive_accounts, :organization, foreign_key: true
  end
end
