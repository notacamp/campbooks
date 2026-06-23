class AddCategoryToDocumentTypes < ActiveRecord::Migration[8.1]
  def change
    add_column :document_types, :category, :string
  end
end
