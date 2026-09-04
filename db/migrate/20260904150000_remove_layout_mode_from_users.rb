# frozen_string_literal: true

class RemoveLayoutModeFromUsers < ActiveRecord::Migration[8.1]
  def change
    if column_exists?(:users, :layout_mode)
      remove_column :users, :layout_mode, :integer, default: 0, null: false
    end
  end
end
