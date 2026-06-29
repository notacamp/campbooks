class AddTagClassificationColumns < ActiveRecord::Migration[8.1]
  def change
    add_column :tags, :kind, :integer, default: 0, null: false
    add_column :tags, :hidden, :boolean, default: false, null: false
    add_column :tags, :classified_at, :datetime
    add_column :tags, :classification_confidence, :float
    add_column :tags, :classification_reason, :string, limit: 255

    add_index :tags, :hidden, where: "hidden = true", name: "index_tags_on_hidden"
    add_index :tags, :classified_at, where: "classified_at IS NULL",
                                      name: "index_tags_on_unclassified"

    # Backfill is intentionally tiny and index-backed (safe even on constrained
    # prod shared memory). The long tail of provider labels is classified
    # out-of-band by Labels::ClassifyLabelJob (rake labels:classify_existing) —
    # never here, so the migration stays fast and boot-time db:prepare is safe.
    reversible do |dir|
      dir.up do
        # Existing provider system labels → hidden system status (kind: system).
        execute(<<~SQL.squish)
          UPDATE tags SET kind = 1, hidden = true, classified_at = CURRENT_TIMESTAMP
          WHERE system_label = true
        SQL

        # Gmail "category" tabs are a subset of system → kind: category.
        execute(<<~SQL.squish)
          UPDATE tags SET kind = 2
          WHERE external_label_id IN
            ('CATEGORY_PERSONAL', 'CATEGORY_SOCIAL', 'CATEGORY_PROMOTIONS',
             'CATEGORY_UPDATES', 'CATEGORY_FORUMS')
        SQL

        # Local (workspace) tags are always genuine user tags — mark decided so
        # they're never picked up by the classification backfill.
        execute(<<~SQL.squish)
          UPDATE tags SET classified_at = CURRENT_TIMESTAMP
          WHERE email_account_id IS NULL
        SQL
      end
    end
  end
end
