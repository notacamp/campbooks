class CreateMailFolderUsers < ActiveRecord::Migration[8.1]
  def change
    create_table :mail_folder_users, id: :uuid do |t|
      t.references :mail_folder, null: false, foreign_key: true, type: :uuid
      t.references :user, null: false, foreign_key: true, type: :uuid
      t.boolean :can_read, null: false, default: true
      t.boolean :can_write, null: false, default: false
      t.boolean :can_manage, null: false, default: false
      t.boolean :owner, null: false, default: false
      t.timestamps
    end

    add_index :mail_folder_users, %i[mail_folder_id user_id], unique: true
  end
end
