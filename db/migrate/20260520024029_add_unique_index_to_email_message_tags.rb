class AddUniqueIndexToEmailMessageTags < ActiveRecord::Migration[8.1]
  def change
    add_index :email_message_tags, [ :email_message_id, :email_tag_id ], unique: true, name: "idx_email_message_tags_unique"
  end
end
