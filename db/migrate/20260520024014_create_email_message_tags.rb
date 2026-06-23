class CreateEmailMessageTags < ActiveRecord::Migration[8.1]
  def change
    create_table :email_message_tags do |t|
      t.references :email_message, null: false, foreign_key: true
      t.references :email_tag, null: false, foreign_key: true

      t.timestamps
    end
  end
end
