class AddBccAddressToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :email_messages, :bcc_address, :text
  end
end
