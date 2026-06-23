class AddJustificationToReminders < ActiveRecord::Migration[8.1]
  # The model's one-sentence rationale for extracting this reminder (e.g. the
  # phrase in the email/document that implies the date). Surfaced in the UI so the
  # user understands why Scout suggested it before confirming.
  def change
    add_column :reminders, :justification, :text
  end
end
