class AddContentHashAndJoinTableToDocuments < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :content_hash, :string
    add_column :documents, :sender_name, :string
    add_index :documents, :content_hash

    create_table :document_email_messages do |t|
      t.references :document, null: false, foreign_key: true
      t.references :email_message, null: false, foreign_key: true
      t.datetime :created_at, null: false
      t.index [ :document_id, :email_message_id ], unique: true, name: "idx_document_email_messages_unique"
    end

    reversible do |dir|
      dir.up do
        execute <<-SQL
          INSERT INTO document_email_messages (document_id, email_message_id, created_at)
          SELECT documents.id, email_messages.id, NOW()
          FROM documents
          INNER JOIN email_messages ON email_messages.zoho_message_id = documents.email_message_id
          WHERE documents.email_message_id IS NOT NULL
          AND documents.email_message_id != ''
        SQL
      end
    end
  end
end
