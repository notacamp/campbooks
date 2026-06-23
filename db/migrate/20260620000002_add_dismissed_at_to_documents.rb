class AddDismissedAtToDocuments < ActiveRecord::Migration[8.1]
  def change
    # dismissed_at: presence = the user flagged this document as junk / not-a-doc
    # during Skim review, so it drops out of the review queue without being deleted
    # (reversible via Undo). Mirrors email Skim's `skimmed_at` — a user-intent flag
    # kept orthogonal to the AI pipeline's `status` state machine.
    add_column :documents, :dismissed_at, :datetime

    # Backs the Skim feed query (status: :review AND dismissed_at IS NULL, per workspace).
    add_index :documents, [ :workspace_id, :status, :dismissed_at ]
  end
end
