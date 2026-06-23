class AddSenderControlsToContacts < ActiveRecord::Migration[8.1]
  def change
    # Per-sender list state (neutral / allowed / blocked) and a star flag.
    # Anchored on Contact because it is the normalized, workspace-scoped sender
    # already linked from email_messages.contact_id.
    add_column :contacts, :list_status, :integer, null: false, default: 0
    add_column :contacts, :starred_at, :datetime
    # Watermark for sender auto-tagging (see ContactAnalysisJob).
    add_column :contacts, :auto_tagged_at, :datetime

    add_index :contacts, [ :workspace_id, :list_status ]
    add_index :contacts, [ :workspace_id, :starred_at ],
              where: "starred_at IS NOT NULL",
              name: "index_contacts_on_workspace_and_starred"
  end
end
