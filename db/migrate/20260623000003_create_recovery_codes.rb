class CreateRecoveryCodes < ActiveRecord::Migration[8.1]
  def change
    create_table :recovery_codes do |t|
      t.references :user, null: false, foreign_key: true
      t.string   :code_digest, null: false  # BCrypt digest of the one-time code
      t.datetime :used_at                    # set when consumed; non-null => spent
      t.timestamps
    end
  end
end
