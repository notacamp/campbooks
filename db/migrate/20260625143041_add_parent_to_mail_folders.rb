class AddParentToMailFolders < ActiveRecord::Migration[8.1]
  def change
    add_reference :mail_folders, :parent, null: true, foreign_key: { to_table: :mail_folders }, index: true
  end
end
