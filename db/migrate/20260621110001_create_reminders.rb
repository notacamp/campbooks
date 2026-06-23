class CreateReminders < ActiveRecord::Migration[8.1]
  # A reminder is a calendar-worthy dated commitment the AI extracted from an
  # email or document, staged for the user to confirm into a real CalendarEvent.
  # It is a first-class entity (not a FeedItem data blob) for two reasons:
  #   1. It must survive the source email being archived — exactly when the
  #      due-date still matters.
  #   2. An invoice arriving as both an email and a PDF attachment must dedup to
  #      one reminder (the unique extraction_fingerprint), which nothing else does.
  def change
    create_table :reminders do |t|
      t.references :workspace, null: false, foreign_key: true

      t.integer  :reminder_type, null: false           # enum — the taxonomy
      t.string   :title, null: false
      t.text     :description
      t.datetime :due_at, null: false                  # stored UTC
      t.boolean  :all_day, null: false, default: false
      t.integer  :status, null: false, default: 0      # pending/confirmed/dismissed/snoozed

      # Polymorphic source: EmailMessage | Document. Single dispatch for
      # accessible_to / still_valid?, and extensible to future sources.
      t.string :source_type, null: false
      t.bigint :source_id,   null: false

      # Set on confirm. Nullable: a confirmed reminder with no connected calendar
      # still exists in-app without a CalendarEvent.
      t.references :calendar_event, foreign_key: true
      t.references :confirmed_by, foreign_key: { to_table: :users }

      t.float    :confidence, null: false, default: 0.0
      t.integer  :amount_cents                          # for payment_due reminders
      t.string   :currency
      t.datetime :snoozed_until

      t.jsonb  :extracted_data, null: false, default: {} # raw AI output, for debugging
      t.string :extraction_fingerprint                   # idempotency key

      t.timestamps
    end

    add_index :reminders, [ :source_type, :source_id ], name: "index_reminders_on_source"
    add_index :reminders, [ :workspace_id, :status, :due_at ], name: "index_reminders_on_workspace_status_due"
    add_index :reminders, :extraction_fingerprint, unique: true,
              where: "extraction_fingerprint IS NOT NULL",
              name: "index_reminders_on_fingerprint"
  end
end
