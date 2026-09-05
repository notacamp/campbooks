# frozen_string_literal: true

class AddAiAskToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :email_messages, :ai_ask, :string
  end
end
