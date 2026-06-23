class AddAdminApprovedToInvitations < ActiveRecord::Migration[8.0]
  def change
    add_column :invitations, :admin_approved, :boolean, default: true, null: false
    add_index :invitations, :admin_approved
  end
end
