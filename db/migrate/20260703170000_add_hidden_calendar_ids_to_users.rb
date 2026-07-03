class AddHiddenCalendarIdsToUsers < ActiveRecord::Migration[8.1]
  def change
    # Calendars the user has hidden from their /calendar view (per-user, display
    # only — syncing stays account-wide). Array of calendar uuid strings;
    # mirrors the dismissed_tours jsonb pattern.
    add_column :users, :hidden_calendar_ids, :jsonb, default: [], null: false
  end
end
