class AddMfaToUsers < ActiveRecord::Migration[8.1]
  def change
    change_table :users, bulk: true do |t|
      t.text     :totp_secret           # encrypted at the app layer (encrypts :totp_secret)
      t.datetime :totp_enabled_at       # presence => TOTP active
      t.datetime :email_otp_enabled_at  # presence => email OTP active
      t.string   :webauthn_id           # stable per-user WebAuthn user handle (base64url)
      t.datetime :mfa_last_totp_at      # window of last accepted TOTP code (replay guard)
    end

    add_index :users, :webauthn_id, unique: true, where: "webauthn_id IS NOT NULL"
  end
end
