class AddViewedAtToDocumentsAndReminders < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :viewed_at, :datetime
    add_column :reminders, :viewed_at, :datetime

    # Backfill existing records as viewed so they don't retroactively
    # light the dots for every historical record.
    reversible do |dir|
      dir.up do
        execute "UPDATE documents SET viewed_at = created_at"
        execute "UPDATE reminders SET viewed_at = created_at"
      end
    end
  end
end
