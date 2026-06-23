class AddUniqueIndexToDocumentTagAssignments < ActiveRecord::Migration[8.1]
  def change
    add_index :document_tag_assignments, [ :document_id, :document_tag_id ], unique: true, name: "idx_doc_tag_assignments_unique"
  end
end
