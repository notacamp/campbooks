class CreateCalendarEvents < ActiveRecord::Migration[8.1]
  def change
    # The events themselves — calendar-side mirror of `email_messages`. Stores
    # concrete dated rows (standalone events and materialized recurrence
    # instances) so agenda/grid range queries hit an index instead of expanding
    # RRULEs in Ruby.
    create_table :calendar_events do |t|
      t.references :calendar, null: false, foreign_key: true

      t.string :provider_event_id, null: false
      t.string :title
      t.text   :description
      t.string :location
      t.string :html_link       # provider event page ("open in Google Calendar")
      t.string :conference_url   # Meet/Zoom join link, for the feed "Join" button

      t.datetime :start_at        # stored UTC
      t.datetime :end_at          # stored UTC
      t.string   :start_time_zone # original IANA zone of start
      t.string   :end_time_zone
      t.boolean  :all_day, null: false, default: false

      t.integer :status, null: false, default: 0 # confirmed: 0, tentative: 1, cancelled: 2
      t.integer :rsvp_status                      # needs_action: 0, accepted: 1, declined: 2, tentative: 3
      t.boolean :is_organizer, null: false, default: false
      t.jsonb   :attendees, null: false, default: [] # [{ email:, name:, rsvp_status: }]

      # Conflict detection + sync-loop avoidance (see Risk 1).
      t.string  :provider_etag
      t.integer :provider_sequence
      t.boolean :outbound_pending, null: false, default: false

      # Recurrence: a master carries the rrule; instances/exceptions point back to
      # it via recurring_event_master_id, with original_start_at on overrides.
      t.string :rrule
      t.references :recurring_event_master,
                   foreign_key: { to_table: :calendar_events, on_delete: :cascade },
                   index: true
      t.datetime :original_start_at

      # Provenance: set when the event was created from an email via the action.
      t.references :source_email_message,
                   foreign_key: { to_table: :email_messages, on_delete: :nullify },
                   index: true

      t.timestamps
    end

    add_index :calendar_events, [ :calendar_id, :provider_event_id ], unique: true,
              name: "index_calendar_events_on_calendar_and_provider_id"
    add_index :calendar_events, :start_at
    add_index :calendar_events, [ :start_at, :end_at ], name: "index_calendar_events_on_range"
    add_index :calendar_events, :status
  end
end
