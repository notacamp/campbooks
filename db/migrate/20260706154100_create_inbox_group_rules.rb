# frozen_string_literal: true

# Persists the rule-based membership for inbox groups. A group can now match
# threads not only because of tags (group_name on Tag), but because of who sent
# the message, which organization the sender belongs to, which document type is
# attached, or via a structured search query. Each row encodes one rule: a group
# name, the rule type, and the primary value (an email address / domain, an
# Organization uuid, a DocumentType uuid, or a query string). Rule types:
#   sender       - value is an email address or @domain
#   organization - value is an Organization id
#   document_type - value is a DocumentType id
#   query        - value is a structured-filter query (from:/tag:/is:… only)
class CreateInboxGroupRules < ActiveRecord::Migration[8.1]
  def change
    create_table :inbox_group_rules, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.string :group_name, null: false
      t.string :rule_type, null: false
      t.string :value, null: false
      t.jsonb :params, null: false, default: {}
      t.timestamps
    end

    add_index :inbox_group_rules, %i[workspace_id group_name],
              name: "idx_inbox_group_rules_on_workspace_and_group"
  end
end
