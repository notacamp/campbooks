class AddIconToMailFolders < ActiveRecord::Migration[8.1]
  def change
    add_column :mail_folders, :icon, :string
  end
end
