class AdjustCalendarEventRecurrence < ActiveRecord::Migration[8.1]
  def change
    # v1 recurrence: Google is pulled with singleEvents=true, which returns
    # concrete dated instances each carrying the series id (recurringEventId).
    # We group instances by that string instead of materializing a separate
    # master row (which would risk duplicating the first occurrence in the grid),
    # so the self-referential master FK is replaced by a plain series-id column.
    add_column :calendar_events, :recurring_event_provider_id, :string
    add_index :calendar_events, :recurring_event_provider_id

    remove_reference :calendar_events, :recurring_event_master,
                     foreign_key: { to_table: :calendar_events }
  end
end
