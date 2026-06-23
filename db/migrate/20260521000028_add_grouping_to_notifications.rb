class AddGroupingToNotifications < ActiveRecord::Migration[8.1]
  def change
    add_column :notifications, :group_key, :string
    add_index :notifications, [ :user_id, :group_key ]
    add_column :notifications, :count, :integer, default: 1, null: false
  end
end
