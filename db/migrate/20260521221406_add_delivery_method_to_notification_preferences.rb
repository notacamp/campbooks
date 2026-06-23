class AddDeliveryMethodToNotificationPreferences < ActiveRecord::Migration[8.1]
  def change
    add_column :notification_preferences, :delivery_method, :integer, default: 0, null: false
  end
end
