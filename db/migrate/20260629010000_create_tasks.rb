class CreateTasks < ActiveRecord::Migration[8.1]
  # A Task is an actionable item the user must *do* to complete — distinct from a
  # CalendarEvent (scheduled time) and a Reminder (a dated commitment like a bill).
  # Tasks are created manually or extracted from an email/document by AI, triaged
  # in Skim, surfaced in the Feed, moved through a status board, assigned to
  # members, labelled (shared Tag), and linked to emails (origin + typed links).
  #
  # UUID primary keys throughout (the post-#89 convention). The polymorphic origin
  # mirrors Reminder#source; the unique extraction_fingerprint dedups re-extraction.
  def change
    create_table :tasks, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :workspace,  null: false, foreign_key: true, type: :uuid
      # Nullable: manual tasks set the creator; AI-extracted/system tasks have no
      # human creator (their provenance is the polymorphic source below).
      t.references :created_by, foreign_key: { to_table: :users, on_delete: :nullify }, type: :uuid

      t.string   :title, null: false
      t.text     :description                             # TipTap HTML
      t.integer  :status,   null: false, default: 0       # suggested/todo/in_progress/blocked/done/cancelled
      t.integer  :priority, null: false, default: 1       # low/normal/high/urgent
      t.datetime :due_at                                  # stored UTC
      t.boolean  :all_day,  null: false, default: false
      t.datetime :completed_at
      t.integer  :position, null: false, default: 0       # board ordering within a status column

      # Polymorphic origin: EmailMessage | Document. Nullable — manual tasks have
      # no source. Additional, typed email relationships live in task_email_links.
      t.string :source_type
      t.uuid   :source_id

      t.float   :confidence,   null: false, default: 0.0  # AI extraction confidence
      t.boolean :ai_suggested, null: false, default: false
      t.text    :justification                            # AI rationale / why this is a task
      t.jsonb   :extracted_data, null: false, default: {} # raw AI output, for debugging
      t.string  :extraction_fingerprint                   # idempotency key

      t.timestamps
    end

    add_index :tasks, [ :source_type, :source_id ], name: "index_tasks_on_source"
    add_index :tasks, [ :workspace_id, :status, :due_at ], name: "index_tasks_on_workspace_status_due"
    add_index :tasks, :extraction_fingerprint, unique: true,
              where: "extraction_fingerprint IS NOT NULL",
              name: "index_tasks_on_fingerprint"

    # Multiple assignees per task (assign members). The creator is tracked
    # separately on tasks.created_by_id.
    create_table :task_assignments, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :task,        null: false, foreign_key: true, type: :uuid
      t.references :user,        null: false, foreign_key: true, type: :uuid
      t.references :assigned_by, foreign_key: { to_table: :users, on_delete: :nullify }, type: :uuid
      t.timestamps
    end
    add_index :task_assignments, [ :task_id, :user_id ], unique: true

    # Typed links to emails beyond the origin (related/reference/follow_up/blocked_by).
    create_table :task_email_links, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :task,          null: false, foreign_key: true, type: :uuid
      t.references :email_message, null: false, foreign_key: true, type: :uuid
      t.references :created_by,    foreign_key: { to_table: :users, on_delete: :nullify }, type: :uuid
      t.integer    :relationship,  null: false, default: 0
      t.timestamps
    end
    add_index :task_email_links, [ :task_id, :email_message_id ], unique: true

    # Labels — the same workspace Tag records emails use, via a dedicated join
    # (mirrors email_message_tags). No change to the Tag model.
    create_table :task_tags, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :task, null: false, foreign_key: true, type: :uuid
      t.references :tag,  null: false, foreign_key: true, type: :uuid
      t.timestamps
    end
    add_index :task_tags, [ :task_id, :tag_id ], unique: true
  end
end
