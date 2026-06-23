class RenameEmailTagsToTags < ActiveRecord::Migration[8.1]
  def change
    rename_table :email_tags, :tags
    rename_column :email_message_tags, :email_tag_id, :tag_id
  end
end
