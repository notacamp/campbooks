class AddSectionSeenAtToUsers < ActiveRecord::Migration[8.1]
  # Backs the primary-nav "action required" dots (Navigation::Attention): a per
  # user map of nav section => ISO8601 timestamp of when they last looked.
  def up
    add_column :users, :section_seen_at, :jsonb, default: {}, null: false

    # Backfill existing users to ship-time for every section, so turning the
    # feature on doesn't retroactively light every dot for established accounts.
    # New users keep the {} default and fall back to created_at in the model.
    now = Time.current.utc.iso8601
    seed = { home: now, mail: now, calendar: now, documents: now, scout: now }.to_json
    execute("UPDATE users SET section_seen_at = #{quote(seed)}::jsonb")
  end

  def down
    remove_column :users, :section_seen_at
  end
end
