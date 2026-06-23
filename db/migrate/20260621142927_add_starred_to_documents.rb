class AddStarredToDocuments < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :starred, :boolean, default: false, null: false
    # Sorts starred-first within a workspace (DocumentsController#index).
    add_index :documents, [ :workspace_id, :starred ]
  end
end
