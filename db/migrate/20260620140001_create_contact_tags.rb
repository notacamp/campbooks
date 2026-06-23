class CreateContactTags < ActiveRecord::Migration[8.1]
  def change
    # Direct sender -> tag association (distinct from Contact's transitive
    # `tags through: :email_messages`). Holds the AI-assigned "what this sender
    # usually sends" tags plus any manual ones.
    create_table :contact_tags do |t|
      t.references :contact, null: false, foreign_key: true
      t.references :tag, null: false, foreign_key: true
      t.integer :source, null: false, default: 0 # auto / manual
      t.float :confidence

      t.timestamps
    end

    add_index :contact_tags, [ :contact_id, :tag_id ], unique: true
  end
end
