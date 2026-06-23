class AddGroupNameToTags < ActiveRecord::Migration[8.1]
  def change
    add_column :tags, :group_name, :string
    add_index :tags, [ :workspace_id, :group_name ]
  end
end
