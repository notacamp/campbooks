class AddContactToEmailMessages < ActiveRecord::Migration[8.0]
  def change
    add_reference :email_messages, :contact, foreign_key: true, null: true
  end
end
