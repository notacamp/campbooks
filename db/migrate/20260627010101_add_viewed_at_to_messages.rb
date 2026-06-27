class AddViewedAtToMessages < ActiveRecord::Migration[8.1]
  def change
    # Email: add viewed_at timestamp. Backfill from skimmed_at (the existing
    # "handled" timestamp) or from updated_at when read = true. if_not_exists +
    # the IS NULL guard keep this safe to re-run / idempotent.
    add_column :email_messages, :viewed_at, :datetime, if_not_exists: true

    reversible do |dir|
      dir.up do
        execute <<~SQL.squish
          UPDATE email_messages
          SET viewed_at = COALESCE(skimmed_at, CASE WHEN read THEN updated_at END)
          WHERE viewed_at IS NULL
        SQL
      end
    end

    # Agent messages: add viewed_at timestamp. Backfill existing AI-authored
    # messages as viewed so they don't retroactively light the Scout dot.
    add_column :agent_messages, :viewed_at, :datetime, if_not_exists: true

    reversible do |dir|
      dir.up do
        execute <<~SQL.squish
          UPDATE agent_messages SET viewed_at = created_at WHERE author_type = 1 AND viewed_at IS NULL
        SQL
      end
    end
  end
end
