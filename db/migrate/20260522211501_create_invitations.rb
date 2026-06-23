class CreateInvitations < ActiveRecord::Migration[8.1]
  def change
    create_table :invitations do |t|
      t.references :organization, null: false, foreign_key: true
      t.string :email, null: false
      t.string :token, null: false
      t.integer :status, null: false, default: 0
      t.references :invited_by, null: false, foreign_key: { to_table: :users }
      t.references :accepted_by, foreign_key: { to_table: :users }
      t.datetime :expires_at, null: false
      t.datetime :accepted_at
      t.timestamps

      t.index :token, unique: true
      t.index [ :email, :organization_id, :status ], name: "idx_invitations_on_email_org_status"
    end
  end
end
