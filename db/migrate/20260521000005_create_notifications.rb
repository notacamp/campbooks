class CreateNotifications < ActiveRecord::Migration[8.1]
  def change
    create_table :notifications do |t|
      t.references :user, null: false, foreign_key: true
      t.string :title, null: false
      t.text :body
      t.string :link_url
      t.boolean :read, null: false, default: false
      t.datetime :read_at

      t.timestamps
    end

    add_index :notifications, [ :user_id, :read, :created_at ]
  end
end
