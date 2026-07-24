# frozen_string_literal: true

# Lets the user wave the parked-draft pill away without deleting the draft:
# a dismissed draft keeps its content but grows no pill until edited again.
class AddDismissedAtToDraftEmails < ActiveRecord::Migration[8.1]
  def change
    add_column :draft_emails, :dismissed_at, :datetime
  end
end
