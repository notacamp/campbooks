class CreateBetaCodes < ActiveRecord::Migration[8.1]
  def change
    create_table :beta_codes do |t|
      t.string :code, null: false
      t.string :label
      t.datetime :redeemed_at
      t.references :redeemed_by, foreign_key: { to_table: :users }
      t.references :created_by, foreign_key: { to_table: :users }
      t.datetime :expires_at

      t.timestamps
    end

    add_index :beta_codes, :code, unique: true
  end
end
