class CreateZohoLabels < ActiveRecord::Migration[8.0]
  def change
    create_table :zoho_labels do |t|
      t.references :email_account, null: false, foreign_key: true
      t.string :zoho_label_id
      t.string :name, null: false
      t.string :color, null: false, default: "#ffd700"
      t.boolean :active, default: true, null: false
      t.timestamps
    end

    add_index :zoho_labels, [ :email_account_id, :name ], unique: true
    add_index :zoho_labels, [ :email_account_id, :zoho_label_id ], unique: true,
              where: "zoho_label_id IS NOT NULL", name: "idx_zoho_labels_on_account_and_zoho_label_id"
  end
end
