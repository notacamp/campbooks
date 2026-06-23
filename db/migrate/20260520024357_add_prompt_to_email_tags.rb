class AddPromptToEmailTags < ActiveRecord::Migration[8.1]
  def change
    add_column :email_tags, :prompt, :text
  end
end
