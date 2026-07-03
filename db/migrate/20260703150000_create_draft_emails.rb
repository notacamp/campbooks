class CreateDraftEmails < ActiveRecord::Migration[8.1]
  def change
    create_table :draft_emails, id: :uuid do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.references :user, null: false, foreign_key: true, type: :uuid
      t.references :email_account, foreign_key: true, type: :uuid
      t.references :in_reply_to, foreign_key: { to_table: :email_messages }, type: :uuid
      t.references :signature, foreign_key: true, type: :uuid
      t.integer :mode, null: false, default: 0
      t.text :to_address
      t.text :cc_address
      t.text :bcc_address
      t.string :subject
      t.text :body
      t.text :quoted_body
      t.jsonb :attachments_json, null: false, default: []

      t.timestamps
    end

    add_index :draft_emails, [ :user_id, :updated_at ]
  end
end
