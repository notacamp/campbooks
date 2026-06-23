class AddEmailThreadToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_reference :email_messages, :email_thread, foreign_key: true
  end
end
