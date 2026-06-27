class AddViewedAtToMessages < ActiveRecord::Migration[8.1]
  def change
    # Email: add viewed_at timestamp. Backfill from skimmed_at (the existing
    # "handled" timestamp) or from updated_at when read = true.
    add_column :email_messages, :viewed_at, :datetime

    reversible do |dir|
      dir.up do
        execute <<~SQL.squish
          UPDATE email_messages
          SET viewed_at = COALESCE(skimmed_at, CASE WHEN read THEN updated_at END)
        SQL
      end
    end

    # Agent messages: add viewed_at timestamp. Backfill existing AI-authored
    # messages as viewed so they don't retroactively light the Scout dot.
    add_column :agent_messages, :viewed_at, :datetime

    reversible do |dir|
      dir.up do
        execute <<~SQL.squish
          UPDATE agent_messages SET viewed_at = created_at WHERE author_type = 1
        SQL
      end
    end
  end
end
