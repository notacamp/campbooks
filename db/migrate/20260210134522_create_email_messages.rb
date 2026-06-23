class CreateEmailMessages < ActiveRecord::Migration[8.1]
  def change
    create_table :email_messages do |t|
      t.string :zoho_message_id, null: false
      t.string :zoho_folder_id
      t.string :from_address
      t.string :subject
      t.datetime :received_at
      t.integer :status, null: false, default: 0

      t.timestamps
    end

    add_index :email_messages, :zoho_message_id, unique: true
    add_index :email_messages, :status
  end
end
