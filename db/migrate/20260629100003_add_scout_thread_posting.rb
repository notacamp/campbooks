class AddScoutThreadPosting < ActiveRecord::Migration[8.1]
  def change
    # Set once Scout has posted a doc link into the originating email thread, so it
    # never double-posts for the same document (Files Phase 3c).
    add_column :documents, :posted_to_thread_at, :datetime
    # Per-workspace opt-in for that behaviour (off by default to stay quiet).
    add_column :workspaces, :scout_thread_posts, :boolean, null: false, default: false
  end
end
