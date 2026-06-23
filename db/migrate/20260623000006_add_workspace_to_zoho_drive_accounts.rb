class AddWorkspaceToZohoDriveAccounts < ActiveRecord::Migration[8.1]
  # ZohoDriveAccount was workspace-less, so its controllers loaded/destroyed
  # records by bare id across tenants. Add a workspace FK so every lookup can be
  # scoped. Nullable at the DB level (the feature has ~no rows); the model
  # requires it for new records.
  def change
    add_reference :zoho_drive_accounts, :workspace, foreign_key: true, index: true
  end
end
