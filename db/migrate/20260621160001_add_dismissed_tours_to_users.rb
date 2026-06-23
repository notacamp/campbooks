class AddDismissedToursToUsers < ActiveRecord::Migration[8.1]
  def change
    # Keys of one-time guided overlays (e.g. the skim intro) the user has seen.
    # Array of strings; mirrors the section_seen_at jsonb pattern.
    add_column :users, :dismissed_tours, :jsonb, default: [], null: false
  end
end
