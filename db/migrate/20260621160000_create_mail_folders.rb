class CreateMailFolders < ActiveRecord::Migration[8.1]
  def change
    create_table :mail_folders do |t|
      t.references :workspace, null: false, foreign_key: true
      t.string :name, null: false
      t.integer :position, null: false, default: 0

      t.timestamps
    end

    # One folder name per workspace, case-insensitive (the chip bar + name-based
    # cross-account filtering key on the name, so duplicates would be ambiguous).
    add_index :mail_folders, "workspace_id, lower(name)",
              unique: true, name: "index_mail_folders_on_workspace_and_lower_name"
  end
end
