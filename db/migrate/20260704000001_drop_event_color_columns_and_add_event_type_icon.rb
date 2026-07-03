class DropEventColorColumnsAndAddEventTypeIcon < ActiveRecord::Migration[8.1]
  def change
    # Color now belongs solely to the owning calendar (Calendar#display_color →
    # CalendarAccount#color); events are distinguished by their event type's
    # icon instead of a color. Existing per-event overrides and per-type colors
    # are deliberately discarded — the new model has no place for them.
    remove_column :calendar_events, :color, :string
    remove_column :event_types, :color, :string
    add_column :event_types, :icon, :string
  end
end
