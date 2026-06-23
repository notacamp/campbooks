class ReplaceDeliveryMethodWithNotifyBooleans < ActiveRecord::Migration[8.1]
  def change
    remove_column :notification_preferences, :delivery_method, :integer, default: 0, null: false
    add_column :notification_preferences, :notify_in_app, :boolean, default: true, null: false
    add_column :notification_preferences, :notify_email, :boolean, default: false, null: false
  end
end
