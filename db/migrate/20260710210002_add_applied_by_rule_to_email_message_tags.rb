# frozen_string_literal: true

# Adds provenance tracking to email_message_tags: when a rule applied the tag
# the row records which rule did it.  on_delete: :nullify so deleting a rule
# does not cascade-delete the tag rows (the tag stays; only the attribution
# is cleared).  A partial index keeps lookups and UndoRun deletes fast while
# the index is absent for the majority of rows (user-applied tags).
class AddAppliedByRuleToEmailMessageTags < ActiveRecord::Migration[8.1]
  def change
    add_reference :email_message_tags, :applied_by_rule,
                  type: :uuid,
                  null: true,
                  foreign_key: { to_table: :email_rules, on_delete: :nullify },
                  index: false

    add_index :email_message_tags, :applied_by_rule_id,
              where: "applied_by_rule_id IS NOT NULL",
              name: "idx_email_message_tags_applied_by_rule"
  end
end
