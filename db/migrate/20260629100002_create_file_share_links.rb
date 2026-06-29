class CreateFileShareLinks < ActiveRecord::Migration[8.1]
  def change
    create_table :file_share_links, id: :uuid do |t|
      t.string :token, null: false
      t.string :shareable_type, null: false
      t.uuid :shareable_id, null: false
      t.references :created_by, foreign_key: { to_table: :users }, type: :uuid
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.datetime :revoked_at
      t.datetime :expires_at
      t.datetime :last_viewed_at
      t.integer :view_count, null: false, default: 0
      t.timestamps
    end

    add_index :file_share_links, :token, unique: true
    add_index :file_share_links, %i[shareable_type shareable_id]
  end
end
