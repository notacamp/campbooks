class AddPushWatchExpiryToEmailAccounts < ActiveRecord::Migration[8.1]
  def change
    add_column :email_accounts, :push_watch_expires_at, :datetime,
               comment: "Gmail users.watch channel expiry (renewed by Emails::WatchRenewalJob); nil = no active push channel"
  end
end
