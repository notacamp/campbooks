class AddMailSyncCursors < ActiveRecord::Migration[8.1]
  # Per-vendor delta-sync cursors, replacing the periodic full re-walk.
  #   - Gmail: users.history.list is mailbox-wide, so one cursor per account.
  #   - Microsoft Graph: messages/delta is per-folder, so a token per folder.
  #   - Zoho: no change feed; a per-folder received-time watermark is the best
  #     it can do (its strategy windows new mail and reconciles on a slower pass).
  def change
    add_column :email_accounts, :history_id, :string
    add_column :email_folders, :delta_token, :string
    add_column :email_folders, :last_synced_at, :datetime
  end
end
