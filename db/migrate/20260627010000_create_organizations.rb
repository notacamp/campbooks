class CreateOrganizations < ActiveRecord::Migration[8.0]
  def change
    create_table :organizations, id: :uuid do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.string :name, null: false
      t.string :domain
      t.text :notes
      t.timestamps
    end

    add_index :organizations, %i[workspace_id name], unique: true
  end
end
