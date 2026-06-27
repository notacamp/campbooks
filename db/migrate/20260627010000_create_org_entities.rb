class CreateOrgEntities < ActiveRecord::Migration[8.1]
  def change
    create_table :organizations, id: :uuid do |t|
      t.references :workspace, null: false, foreign_key: true
      t.string :name, null: false
      t.string :domain
      t.text :notes
      t.timestamps
    end
    add_index :organizations, %i[workspace_id name], unique: true
  end
end
