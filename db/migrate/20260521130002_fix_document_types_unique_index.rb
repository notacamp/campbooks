class FixDocumentTypesUniqueIndex < ActiveRecord::Migration[8.1]
  def change
    remove_index :document_types, name: "index_document_types_on_name"
    add_index :document_types, [ :organization_id, :name ], unique: true,
              name: "index_document_types_on_org_and_name"
  end
end
