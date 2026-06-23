class AddColorToCalendarEvents < ActiveRecord::Migration[8.1]
  def change
    # Per-event color override (hex, e.g. "#dc2127"). Nullable: when blank the event
    # inherits its calendar's color (CalendarEvent#display_color). Syncs two-way with
    # the provider — Google maps it to/from colorId, Zoho to its event `color`.
    add_column :calendar_events, :color, :string
  end
end
