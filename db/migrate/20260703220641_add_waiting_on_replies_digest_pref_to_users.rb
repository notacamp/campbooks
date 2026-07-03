class AddWaitingOnRepliesDigestPrefToUsers < ActiveRecord::Migration[8.1]
  def change
    # Opt-out (default on): the daily "waiting on replies" email digest reaches
    # users out-of-app. Empty/no-due digests are skipped, so an on-by-default
    # flag never sends a pointless email; users toggle it in Settings → Notifications.
    add_column :users, :email_on_waiting_on_replies_digest, :boolean, default: true, null: false
  end
end
