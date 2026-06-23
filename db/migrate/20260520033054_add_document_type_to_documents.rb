class AddDocumentTypeToDocuments < ActiveRecord::Migration[8.1]
  def change
    create_table :document_types do |t|
      t.string :name, null: false
      t.string :color, null: false
      t.text :prompt
      t.timestamps
    end
    add_index :document_types, :name, unique: true

    add_reference :documents, :document_type, foreign_key: true

    drop_table :document_tag_assignments
    drop_table :document_tags
  end
end
