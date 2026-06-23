class CreateCalendarAccounts < ActiveRecord::Migration[8.1]
  def change
    # A connected calendar provider grant — the calendar-side mirror of
    # `email_accounts`. Reuses the same Google/Zoho OAuth apps but a distinct
    # grant (calendar scopes), so it carries its own encrypted refresh token.
    create_table :calendar_accounts do |t|
      t.references :workspace, null: false, foreign_key: true

      t.string  :email_address, null: false
      t.integer :provider, null: false, default: 0 # google: 0, zoho: 1
      t.string  :provider_account_id
      t.string  :refresh_token, null: false        # encrypted at the app layer
      t.string  :name
      t.string  :color, null: false, default: "#3b82f6"

      t.boolean :active, null: false, default: true

      # Slot-lock for the sync job (see CalendarAccount::SCAN_STALE_AFTER), mirroring
      # the email scan claim: `scanning` is the flag, `scan_started_at` ages it out.
      t.boolean  :scanning, null: false, default: false
      t.datetime :scan_started_at
      t.datetime :last_scanned_at

      t.timestamps
    end

    # The same login can legitimately be linked twice under different providers,
    # so uniqueness is per (email, provider) rather than email alone.
    add_index :calendar_accounts, [ :email_address, :provider ], unique: true,
              name: "index_calendar_accounts_on_email_and_provider"
    add_index :calendar_accounts, :provider
    # Fast path for the recurring sync, which iterates active accounts.
    add_index :calendar_accounts, :active, where: "active = true",
              name: "index_calendar_accounts_active"
  end
end
