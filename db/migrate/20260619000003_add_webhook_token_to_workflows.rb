class AddWebhookTokenToWorkflows < ActiveRecord::Migration[8.1]
  def change
    add_column :workflows, :webhook_token, :string
    add_index :workflows, :webhook_token, unique: true
  end
end
