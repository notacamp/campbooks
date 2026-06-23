class CreateCalendarWebhookChannels < ActiveRecord::Migration[8.1]
  def change
    # Registered Google `events.watch` push channels. Needed to verify inbound
    # webhook tokens, renew channels before they expire, and stop old ones.
    create_table :calendar_webhook_channels do |t|
      t.references :calendar, null: false, foreign_key: true

      t.string   :provider_channel_id, null: false # the `id` we sent to Google
      t.string   :provider_resource_id             # returned by Google; needed for stop()
      t.string   :channel_token, null: false       # our secret, verified on inbound POSTs
      t.datetime :expires_at

      t.timestamps
    end

    add_index :calendar_webhook_channels, :provider_channel_id, unique: true
    add_index :calendar_webhook_channels, :expires_at
  end
end
