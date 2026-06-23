class CreateDocumentTags < ActiveRecord::Migration[8.1]
  def change
    create_table :document_tags do |t|
      t.string :name
      t.string :color
      t.text :prompt

      t.timestamps
    end
  end
end
