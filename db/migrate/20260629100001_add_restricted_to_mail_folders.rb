class AddRestrictedToMailFolders < ActiveRecord::Migration[8.1]
  def change
    # Open by default (= today's workspace-global visibility). A folder becomes
    # private — visible only to its members + workspace admins — when toggled on.
    add_column :mail_folders, :restricted, :boolean, null: false, default: false
  end
end
