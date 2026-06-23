class AddSkimStateToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    # pinned_at: presence = the email is in Skim's "Priority" lane (user-promoted,
    # overrides time ordering). The timestamp preserves promote order.
    add_column :email_messages, :pinned_at, :datetime
    # skimmed_at: presence = the user has addressed this email in Skim (Keep), so
    # the feed should not surface it again. Archived mail already leaves the inbox
    # folder scope; this covers "Keep" (kept in the inbox but handled).
    add_column :email_messages, :skimmed_at, :datetime

    add_index :email_messages, :pinned_at
    add_index :email_messages, :skimmed_at
  end
end
