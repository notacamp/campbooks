class AddCcAddressToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :email_messages, :cc_address, :text
  end
end
