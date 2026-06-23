# A registered passkey / FIDO2 security key used as a second factor. `external_id`
# is the credential's own base64url id (WebAuthn's stable handle); `public_key` is
# the COSE-encoded key the gem verifies assertions against; `sign_count` is the
# authenticator's monotonic counter (cloning detection — must never go backwards).
class WebauthnCredential < ApplicationRecord
  belongs_to :user

  validates :external_id, :public_key, presence: true
  validates :external_id, uniqueness: true
end
