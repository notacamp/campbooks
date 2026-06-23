class AddZohoFlagToEmailMessages < ActiveRecord::Migration[8.0]
  def change
    add_column :email_messages, :zoho_flag, :string
  end
end
