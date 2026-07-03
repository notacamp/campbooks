class AddComposeDefaultToUsers < ActiveRecord::Migration[8.1]
  def change
    # Where a brand-new email opens (C key, Compose button, Cmd+K):
    # 0 = the Desk (full page), 1 = the Dock (bottom sheet).
    add_column :users, :compose_default, :integer, default: 0, null: false
  end
end
