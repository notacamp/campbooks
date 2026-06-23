class CreateDocumentTagAssignments < ActiveRecord::Migration[8.1]
  def change
    create_table :document_tag_assignments do |t|
      t.references :document, null: false, foreign_key: true
      t.references :document_tag, null: false, foreign_key: true

      t.timestamps
    end
  end
end
