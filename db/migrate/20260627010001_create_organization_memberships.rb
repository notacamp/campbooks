class CreateOrganizationMemberships < ActiveRecord::Migration[8.1]
  def change
    create_table :organization_memberships, id: :uuid do |t|
      t.references :person, null: false, foreign_key: true
      t.references :organization, null: false, foreign_key: true, type: :uuid
      t.integer :status, null: false, default: 0
      t.timestamps
    end
    add_index :organization_memberships, %i[person_id organization_id], unique: true
    add_index :organization_memberships, %i[organization_id status]
  end
end
