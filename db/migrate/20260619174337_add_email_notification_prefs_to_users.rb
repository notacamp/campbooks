class AddEmailNotificationPrefsToUsers < ActiveRecord::Migration[8.1]
  def change
    add_column :users, :email_on_mention, :boolean, default: true, null: false
    add_column :users, :email_on_thread_activity, :boolean, default: true, null: false
  end
end
