class CreateSignatures < ActiveRecord::Migration[8.1]
  def change
    create_table :signatures do |t|
      t.references :user, null: false, foreign_key: true
      t.string :name, null: false
      t.text :content, null: false
      t.boolean :is_default, default: false, null: false
      t.timestamps
    end

    add_index :signatures, [ :user_id, :is_default ]
    add_index :signatures, [ :user_id, :name ], unique: true
  end
end
