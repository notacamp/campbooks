class CreateTemplates < ActiveRecord::Migration[8.1]
  def change
    create_table :templates do |t|
      t.string :name, null: false
      t.text :description
      t.jsonb :data, null: false, default: {}
      t.timestamps
    end
    add_index :templates, :name, unique: true
  end
end
