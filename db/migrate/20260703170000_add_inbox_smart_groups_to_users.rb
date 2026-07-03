class AddInboxSmartGroupsToUsers < ActiveRecord::Migration[8.1]
  def change
    # Per-user smart-group prefs ({"enabled" => bool, "<bucket>" => bool}).
    # Missing keys mean enabled, so the empty default turns the feature ON for
    # everyone without baking today's bucket list into row data.
    add_column :users, :inbox_smart_groups, :jsonb, default: {}, null: false
  end
end
