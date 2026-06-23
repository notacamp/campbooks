class AddReplyStatusToEmailComments < ActiveRecord::Migration[8.1]
  def change
    add_column :email_comments, :reply_status, :integer, default: 0, null: false
    add_index :email_comments, [ :reply_status, :created_at ], name: "idx_email_comments_pending_replies"
  end
end
