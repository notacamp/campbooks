class AddReadToEmailMessages < ActiveRecord::Migration[8.0]
  def change
    add_column :email_messages, :read, :boolean, default: false, null: false
    add_index :email_messages, :read
  end
end
