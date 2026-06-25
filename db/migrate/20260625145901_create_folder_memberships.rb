class CreateFolderMemberships < ActiveRecord::Migration[8.1]
  # The Stage 3 "filesystem" join: a folder can contain heterogeneous content
  # (documents now; emails / reminders later) via a polymorphic folderable.
  def change
    create_table :folder_memberships do |t|
      t.references :mail_folder, null: false, foreign_key: true
      t.references :folderable, polymorphic: true, null: false
      t.integer :position, null: false, default: 0
      t.timestamps
    end
    add_index :folder_memberships, [ :mail_folder_id, :folderable_type, :folderable_id ],
              unique: true, name: "index_folder_memberships_unique"
  end
end
