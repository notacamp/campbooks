class AddCategoryToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    add_column :email_messages, :category, :string
    add_index :email_messages, :category
    add_column :email_messages, :category_confidence, :float
    add_column :email_messages, :categorized_at, :datetime
  end
end
