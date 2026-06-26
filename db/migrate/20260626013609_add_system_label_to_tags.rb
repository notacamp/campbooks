class AddSystemLabelToTags < ActiveRecord::Migration[8.1]
  def change
    add_column :tags, :system_label, :boolean, default: false, null: false
    add_index :tags, :system_label, where: "system_label = true"
  end
end
