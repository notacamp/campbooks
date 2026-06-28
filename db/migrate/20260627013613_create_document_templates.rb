class CreateDocumentTemplates < ActiveRecord::Migration[8.1]
  def change
    create_table :document_templates do |t|
      t.references :workspace, null: false, foreign_key: true
      t.string :name, null: false
      t.text :description
      t.text :html_content, null: false, default: ""
      t.jsonb :variables_schema, null: false, default: []
      t.integer :ai_status, null: false, default: 0
      t.jsonb :ai_provenance, null: false, default: {}
      t.timestamps
    end
    add_index :document_templates, %i[workspace_id name]
  end
end
