class CreateNotificationPreferences < ActiveRecord::Migration[8.1]
  def change
    create_table :notification_preferences do |t|
      t.references :user, null: false, foreign_key: true
      t.integer :kind, null: false
      t.references :tag, foreign_key: true
      t.references :document_type, foreign_key: true
      t.boolean :enabled, null: false, default: true

      t.timestamps
    end

    add_index :notification_preferences, [ :user_id, :kind, :tag_id ],
      unique: true, where: "tag_id IS NOT NULL", name: "idx_notification_prefs_user_kind_tag"
    add_index :notification_preferences, [ :user_id, :kind, :document_type_id ],
      unique: true, where: "document_type_id IS NOT NULL", name: "idx_notification_prefs_user_kind_doctype"
  end
end
