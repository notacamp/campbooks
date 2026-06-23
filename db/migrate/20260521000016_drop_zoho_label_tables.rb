class DropZohoLabelTables < ActiveRecord::Migration[8.0]
  def change
    drop_table :email_message_zoho_labels, if_exists: true
    drop_table :zoho_labels, if_exists: true
  end
end
