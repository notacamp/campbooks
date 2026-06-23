class AddZohoFieldsToTags < ActiveRecord::Migration[8.0]
  def change
    add_column :tags, :zoho_label_id, :string
    add_column :tags, :email_account_id, :bigint
    add_column :tags, :source, :integer, default: 0, null: false

    add_index :tags, :zoho_label_id
    add_index :tags, :email_account_id
    add_index :tags, [ :email_account_id, :name ], unique: true,
              where: "email_account_id IS NOT NULL", name: "idx_tags_on_account_and_name"
    add_index :tags, [ :email_account_id, :zoho_label_id ], unique: true,
              where: "zoho_label_id IS NOT NULL", name: "idx_tags_on_account_and_zoho_label_id"
  end
end
