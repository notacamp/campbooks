class CreateTaskDocuments < ActiveRecord::Migration[8.1]
  # Attach workspace Documents to a Task (e.g. the invoice a task is about).
  # Mirrors task_email_links; UUID-native (the post-#89 convention).
  def change
    create_table :task_documents, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :task,       null: false, foreign_key: true, type: :uuid
      t.references :document,   null: false, foreign_key: true, type: :uuid
      t.references :created_by, foreign_key: { to_table: :users, on_delete: :nullify }, type: :uuid
      t.timestamps
    end
    add_index :task_documents, [ :task_id, :document_id ], unique: true
  end
end
