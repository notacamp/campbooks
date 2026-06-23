class AddAutoStarToDocumentTypes < ActiveRecord::Migration[8.1]
  def change
    # When set, documents classified as this type are starred automatically.
    add_column :document_types, :auto_star, :boolean, default: false, null: false
  end
end
