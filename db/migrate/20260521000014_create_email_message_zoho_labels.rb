class CreateEmailMessageZohoLabels < ActiveRecord::Migration[8.0]
  def change
    create_table :email_message_zoho_labels do |t|
      t.references :email_message, null: false, foreign_key: true
      t.references :zoho_label, null: false, foreign_key: true
      t.timestamps
    end

    add_index :email_message_zoho_labels, [ :email_message_id, :zoho_label_id ], unique: true,
              name: "idx_email_message_zoho_labels_unique"
  end
end
