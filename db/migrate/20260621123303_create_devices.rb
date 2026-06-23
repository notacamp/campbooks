class CreateDevices < ActiveRecord::Migration[8.1]
  def change
    create_table :devices do |t|
      t.references :user, null: false, foreign_key: true
      # 0 = ios (APNs), 1 = android (FCM) — see Device#platform enum.
      t.integer :platform, null: false
      # APNs device token (hex) or FCM registration token. Unique per install.
      t.string :token, null: false
      t.string :app_version
      t.datetime :last_active_at

      t.timestamps
    end

    add_index :devices, :token, unique: true
    add_index :devices, [ :user_id, :platform ]
  end
end
