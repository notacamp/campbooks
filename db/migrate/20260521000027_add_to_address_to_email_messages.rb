class AddToAddressToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :email_messages, :to_address, :string
  end
end
