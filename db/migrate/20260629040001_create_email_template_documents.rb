class CreateEmailTemplateDocuments < ActiveRecord::Migration[8.1]
  def change
    create_table :email_template_documents, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :email_template, null: false, foreign_key: true, type: :uuid
      t.references :document_template, null: false, foreign_key: true, type: :uuid
      t.integer :position, null: false, default: 0
      t.timestamps
    end
    add_index :email_template_documents, %i[email_template_id document_template_id],
              unique: true, name: "idx_email_template_documents_unique"
  end
end
