class AddZohoUidToUsers < ActiveRecord::Migration[8.1]
  def change
    add_column :users, :zoho_uid, :string
  end
end
