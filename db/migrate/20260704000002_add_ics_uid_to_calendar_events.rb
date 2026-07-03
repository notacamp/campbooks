class AddIcsUidToCalendarEvents < ActiveRecord::Migration[8.1]
  def change
    # The source VEVENT UID for events created by the .ics importer. Dedup must
    # live on its own column: the temp "local-…" provider_event_id gets replaced
    # by the provider's real id after the outbound create, so it can't carry the
    # fingerprint. Unique per calendar, only where present.
    add_column :calendar_events, :ics_uid, :string
    add_index :calendar_events, [ :calendar_id, :ics_uid ], unique: true,
              where: "ics_uid IS NOT NULL", name: "index_calendar_events_on_calendar_and_ics_uid"
  end
end
