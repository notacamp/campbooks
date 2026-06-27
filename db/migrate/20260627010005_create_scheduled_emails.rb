class CreateScheduledEmails < ActiveRecord::Migration[8.1]
  def change
    create_table :scheduled_emails, id: :uuid do |t|
      t.references :workspace, null: false, foreign_key: true
      t.references :email_account, null: false, foreign_key: true
      t.references :created_by, null: false, foreign_key: { to_table: :users }
      t.string :to_address, null: false
      t.string :cc_address
      t.string :bcc_address
      t.string :subject, null: false
      t.text :body, null: false
      t.datetime :scheduled_at, null: false
      t.string :rrule
      t.datetime :last_sent_at
      t.datetime :next_occurrence_at
      t.integer :status, default: 0, null: false
      t.jsonb :template_context, default: {}, null: false

      t.timestamps
    end

    add_index :scheduled_emails, [:workspace_id, :status]
    add_index :scheduled_emails, :next_occurrence_at, where: "status = 0", name: "idx_scheduled_emails_pending_next_occurrence"
    add_index :scheduled_emails, :scheduled_at, where: "status = 0", name: "idx_scheduled_emails_pending_scheduled_at"
  end
end
