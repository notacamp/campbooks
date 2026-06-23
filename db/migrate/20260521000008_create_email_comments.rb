class CreateEmailComments < ActiveRecord::Migration[8.1]
  def change
    create_table :email_comments do |t|
      t.references :email_thread, null: false, foreign_key: true
      t.references :user, null: true, foreign_key: true
      t.text :content, null: false
      t.integer :author_type, null: false, default: 0

      t.timestamps
    end

    add_index :email_comments, [ :email_thread_id, :created_at ]
  end
end
