class AddExtractionSchemaToDocumentTypes < ActiveRecord::Migration[8.1]
  def change
    add_column :document_types, :extraction_schema, :jsonb
  end
end
