# frozen_string_literal: true

# Each workspace tag can point at a provider label in one or more connected
# email accounts. This is the pointer table — it lets a single workspace-owned
# tag represent the same label across multiple mailboxes (or be linked later by
# the user via the label-review flow). Additive: all existing columns on `tags`
# (email_account_id, external_label_id) are left intact so existing sync code
# continues to work unchanged.
class CreateTagAccountLinks < ActiveRecord::Migration[8.1]
  def change
    create_table :tag_account_links, id: :uuid do |t|
      t.references :tag,           null: false, foreign_key: true,           type: :uuid
      t.references :email_account, null: false, foreign_key: true,           type: :uuid
      t.string     :provider_label_id,   null: false
      t.string     :provider_label_name

      t.timestamps
    end

    # Each tag points at at most one label per account.
    add_index :tag_account_links, [ :tag_id, :email_account_id ],
              unique: true, name: "idx_tag_account_links_tag_account"
    # Each provider label in an account maps to at most one workspace tag.
    add_index :tag_account_links, [ :email_account_id, :provider_label_id ],
              unique: true, name: "idx_tag_account_links_account_label"
  end
end
