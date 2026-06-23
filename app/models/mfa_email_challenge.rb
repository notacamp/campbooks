# A short-lived, server-side email one-time code used during the login challenge
# (distinct from the registration-signup OTP, which lives in the Rails session).
# A DB row — rather than a signed token — lets us cap attempts and expire/replace
# it server-side. At most one live challenge per user (unique index on user_id).
class MfaEmailChallenge < ApplicationRecord
  belongs_to :user

  MAX_ATTEMPTS = 5
  TTL = 10.minutes
  CODE_RANGE = 1_000_000 # 6 digits

  # Create or replace the user's live challenge with a fresh code. Returns
  # [challenge, plaintext] — the caller emails the plaintext via VerificationMailer.
  def self.start_for!(user)
    code = format("%06d", SecureRandom.random_number(CODE_RANGE))
    # find_or_initialize_by hits the DB each call (no association cache), so a
    # second send updates the one row instead of racing the unique index.
    challenge = find_or_initialize_by(user_id: user.id)
    challenge.update!(
      code_digest: BCrypt::Password.create(code),
      attempts: 0,
      expires_at: TTL.from_now
    )
    [ challenge, code ]
  end

  def expired?
    expires_at < Time.current
  end

  def attempts_exhausted?
    attempts >= MAX_ATTEMPTS
  end

  # Constant-time compare. On a miss, burns an attempt. Callers must check
  # expired?/attempts_exhausted? first (the controller does, then re-sends/aborts).
  def verify(plaintext)
    if BCrypt::Password.new(code_digest) == plaintext.to_s.strip
      true
    else
      increment!(:attempts)
      false
    end
  rescue BCrypt::Errors::InvalidHash
    false
  end
end
