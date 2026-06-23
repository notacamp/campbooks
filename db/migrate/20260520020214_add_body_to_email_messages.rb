class AddBodyToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :email_messages, :body, :text
  end
end
