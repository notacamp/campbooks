class CreateContactEmailAliases < ActiveRecord::Migration[8.0]
  def change
    create_table :contact_email_aliases do |t|
      t.references :contact, null: false, foreign_key: true
      t.string :email, null: false

      t.timestamps
    end

    add_index :contact_email_aliases, :email, unique: true
  end
end
