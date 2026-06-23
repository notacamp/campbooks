class AddMetadataToDocuments < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :metadata, :jsonb
  end
end
