class CreateLearningDecisions < ActiveRecord::Migration[8.1]
  def change
    create_table :learning_decisions, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.string     :domain, null: false                                  # "email_skim" | "tag_suggestion" | ...
      t.references :workspace, null: false, type: :uuid, foreign_key: true, index: false
      t.references :user, null: false, type: :uuid, foreign_key: true, index: false
      t.string     :label, null: false                                   # the human's choice / verdict
      t.uuid       :contact_id                                           # common email signal (FK)
      t.string     :sender_domain                                        # common email signal
      t.string     :category                                             # common email signal (skim)
      t.string     :subject_type                                         # polymorphic audit link (nullable)
      t.uuid       :subject_id
      t.jsonb      :signals, null: false, default: {}                    # domain-specific overflow (tag_name, reminder_type, ...)
      t.timestamps
    end

    add_foreign_key :learning_decisions, :contacts, column: :contact_id

    # The two composite indexes serve both scopings; each is one B-tree range scan
    # for the hot "domain + owner + window" bulk read. workspace_id/user_id alone
    # are covered as index prefixes, so no redundant single-column index is added.
    add_index :learning_decisions, [ :user_id, :domain, :created_at ],
              name: "index_learning_decisions_on_user_domain_time"
    add_index :learning_decisions, [ :workspace_id, :domain, :created_at ],
              name: "index_learning_decisions_on_workspace_domain_time"
    add_index :learning_decisions, :contact_id
  end
end
