class AddViewedAtToDocumentsAndReminders < ActiveRecord::Migration[8.1]
  def change
    # if_not_exists guards a fresh DB set up via db:schema:load from a schema that
    # already carried these columns (they predate this PR in the committed
    # schema.rb): incremental migrate then no-ops the add instead of erroring.
    add_column :documents, :viewed_at, :datetime, if_not_exists: true
    add_column :reminders, :viewed_at, :datetime, if_not_exists: true

    # Backfill existing records as viewed so they don't retroactively light the
    # dots for every historical record. Only touch un-set rows so the backfill is
    # safe to re-run.
    reversible do |dir|
      dir.up do
        execute "UPDATE documents SET viewed_at = created_at WHERE viewed_at IS NULL"
        execute "UPDATE reminders SET viewed_at = created_at WHERE viewed_at IS NULL"
      end
    end
  end
end
