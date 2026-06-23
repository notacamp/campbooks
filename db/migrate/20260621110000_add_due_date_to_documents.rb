class AddDueDateToDocuments < ActiveRecord::Migration[8.1]
  # The "act by" horizon for a document: an invoice's payment due date, a policy
  # renewal date, a contract expiry. Until now these only lived inside the
  # `metadata` jsonb (if the AI emitted them at all). `document_date` is the
  # document's own date; `period_start/end` its coverage window — neither is the
  # deadline. Date-granular: document deadlines are never time-specific.
  def change
    add_column :documents, :due_date, :date

    add_index :documents, [ :workspace_id, :due_date ],
              where: "due_date IS NOT NULL",
              name: "index_documents_on_workspace_and_due_date"
  end
end
