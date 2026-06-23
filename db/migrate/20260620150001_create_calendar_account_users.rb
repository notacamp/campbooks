class CreateCalendarAccountUsers < ActiveRecord::Migration[8.1]
  def change
    # Per-user sharing for a calendar account — mirror of `email_account_users`.
    # `can_write` (create/edit/delete/RSVP) takes the place of email's `can_send`.
    create_table :calendar_account_users do |t|
      t.references :calendar_account, null: false, foreign_key: true
      t.references :user, null: false, foreign_key: true

      t.boolean :can_read,   null: false, default: true
      t.boolean :can_write,  null: false, default: false
      t.boolean :can_manage, null: false, default: false
      t.boolean :owner,      null: false, default: false

      t.timestamps
    end

    add_index :calendar_account_users, [ :calendar_account_id, :user_id ], unique: true,
              name: "index_calendar_account_users_on_account_and_user"
  end
end
