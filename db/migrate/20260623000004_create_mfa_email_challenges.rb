class CreateMfaEmailChallenges < ActiveRecord::Migration[8.1]
  def change
    create_table :mfa_email_challenges do |t|
      t.references :user, null: false, foreign_key: true, index: { unique: true } # one live challenge per user
      t.string   :code_digest, null: false  # BCrypt digest of the emailed code
      t.integer  :attempts, null: false, default: 0
      t.datetime :expires_at, null: false
      t.timestamps
    end

    add_index :mfa_email_challenges, :expires_at
  end
end
