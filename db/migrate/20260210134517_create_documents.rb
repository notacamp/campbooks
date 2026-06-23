class CreateDocuments < ActiveRecord::Migration[8.1]
  def change
    create_table :documents do |t|
      t.integer :document_type, null: false, default: 0
      t.integer :status, null: false, default: 0
      t.string :vendor_name
      t.string :vendor_nif
      t.date :document_date
      t.string :invoice_number
      t.integer :amount_cents
      t.string :currency, default: "EUR"
      t.integer :tax_amount_cents
      t.decimal :tax_rate, precision: 5, scale: 2
      t.text :description
      t.integer :source, null: false, default: 0
      t.string :email_message_id
      t.string :canonical_filename
      t.string :workdrive_file_id
      t.datetime :uploaded_to_workdrive_at
      t.jsonb :ai_extraction_data, default: {}
      t.float :ai_confidence_score
      t.integer :ai_processing_attempts, default: 0
      t.references :reviewed_by, null: true, foreign_key: { to_table: :users }
      t.datetime :reviewed_at

      t.timestamps
    end

    add_index :documents, :document_type
    add_index :documents, :status
    add_index :documents, :source
    add_index :documents, :document_date
    add_index :documents, :vendor_nif
    add_index :documents, :email_message_id
  end
end
