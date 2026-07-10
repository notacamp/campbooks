# frozen_string_literal: true

# Join table between EmailRule and Tag.  Both sides cascade-delete so tags and
# rules can be removed independently without leaving orphan join rows.
class CreateEmailRuleTags < ActiveRecord::Migration[8.1]
  def change
    create_table :email_rule_tags, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :email_rule, null: false,
                   foreign_key: { on_delete: :cascade }, type: :uuid
      t.references :tag, null: false,
                   foreign_key: { on_delete: :cascade }, type: :uuid

      t.timestamps
    end

    add_index :email_rule_tags, %i[email_rule_id tag_id],
              unique: true,
              name: "idx_email_rule_tags_unique"
  end
end
