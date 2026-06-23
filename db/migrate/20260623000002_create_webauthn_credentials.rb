class CreateWebauthnCredentials < ActiveRecord::Migration[8.1]
  def change
    create_table :webauthn_credentials do |t|
      t.references :user, null: false, foreign_key: true
      t.string   :external_id, null: false  # credential.id (base64url) — WebAuthn's stable identifier
      t.text     :public_key, null: false   # COSE-encoded public key (serialized by the webauthn gem)
      t.integer  :sign_count, null: false, default: 0
      t.string   :nickname                  # user-chosen label, e.g. "YubiKey 5"
      t.datetime :last_used_at
      t.timestamps
    end

    add_index :webauthn_credentials, :external_id, unique: true
  end
end
