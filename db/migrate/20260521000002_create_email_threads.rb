class CreateEmailThreads < ActiveRecord::Migration[8.1]
  def change
    create_table :email_threads do |t|
      t.string :subject, null: false
      t.references :email_account, null: false, foreign_key: true

      t.timestamps
    end

    add_index :email_threads, [ :subject, :email_account_id ], unique: true
  end
end
