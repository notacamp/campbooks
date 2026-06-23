class CreateSignupRequests < ActiveRecord::Migration[8.0]
  def change
    create_table :signup_requests do |t|
      t.string :email, null: false
      t.string :name, null: false
      t.string :token, null: false
      t.integer :status, default: 0, null: false
      t.references :reviewed_by, foreign_key: { to_table: :users }
      t.datetime :reviewed_at
      t.references :accepted_by, foreign_key: { to_table: :users }
      t.timestamps
    end

    add_index :signup_requests, :token, unique: true
    add_index :signup_requests, [ :email, :status ]
  end
end
