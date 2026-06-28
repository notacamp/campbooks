class AddAttentionDotIndexes < ActiveRecord::Migration[8.1]
  # Navigation::Attention runs one EXISTS-per-section on every authenticated page
  # render (the nav is global). These partial indexes keep each dot query an index
  # probe instead of a table scan: a row drops out of the index the moment it's
  # viewed/seen, so the indexes only ever hold the handful of un-viewed rows that
  # actually light a dot.
  def change
    add_index :email_messages, :email_account_id,
              where: "viewed_at IS NULL",
              name: "index_email_messages_on_account_unviewed"

    add_index :agent_messages, :agent_thread_id,
              where: "viewed_at IS NULL",
              name: "index_agent_messages_on_thread_unviewed"

    add_index :documents, :workspace_id,
              where: "viewed_at IS NULL",
              name: "index_documents_on_workspace_unviewed"

    add_index :reminders, :workspace_id,
              where: "viewed_at IS NULL",
              name: "index_reminders_on_workspace_unviewed"

    add_index :feed_items, :user_id,
              where: "seen_at IS NULL AND dismissed_at IS NULL AND acted_at IS NULL",
              name: "index_feed_items_on_user_unseen_active"
  end
end
