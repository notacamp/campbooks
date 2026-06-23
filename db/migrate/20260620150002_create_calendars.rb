class CreateCalendars < ActiveRecord::Migration[8.1]
  def change
    # One row per provider calendar within an account (Personal, Work, Holidays,
    # shared team calendars…). The analogue of `email_folders`: per-container
    # metadata + the incremental sync cursor, kept off the event rows.
    create_table :calendars do |t|
      t.references :calendar_account, null: false, foreign_key: true

      t.string :provider_calendar_id, null: false
      t.string :name, null: false
      t.text   :description
      t.string :color
      t.string :time_zone

      t.boolean :is_primary,  null: false, default: false
      t.boolean :is_writable, null: false, default: false
      # User toggle: only calendars with `syncing = true` are pulled by the job.
      t.boolean :syncing,     null: false, default: false

      # Google `syncToken` (opaque) for incremental pulls; Zoho has no token, so
      # `last_event_sync_at` drives its `modifiedSince` instead.
      t.string   :sync_token
      t.datetime :last_event_sync_at
      # Bounded materialization window (see Risk 3): events outside it aren't stored.
      t.datetime :sync_window_start
      t.datetime :sync_window_end

      t.timestamps
    end

    add_index :calendars, [ :calendar_account_id, :provider_calendar_id ], unique: true,
              name: "index_calendars_on_account_and_provider_id"
    add_index :calendars, :syncing, where: "syncing = true",
              name: "index_calendars_syncing"
  end
end
