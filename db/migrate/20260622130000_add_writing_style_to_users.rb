class AddWritingStyleToUsers < ActiveRecord::Migration[8.1]
  def change
    # Personal voice for Scout's reply drafts. writing_style is user-authored;
    # writing_style_learned is auto-derived from the user's sent mail and
    # augmented/overridden by the manual field.
    add_column :users, :writing_style, :text
    add_column :users, :writing_style_learned, :text
    add_column :users, :writing_style_updated_at, :datetime
  end
end
