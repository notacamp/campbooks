# One-time backup codes for signing in when the user can't reach their other
# factors. Only bcrypt digests are stored; the plaintext is shown exactly once at
# generation. Codes are single-use (stamped via `used_at`) and matched leniently
# (case-insensitive, hyphens/spaces ignored) so a hand-typed code still works.
class RecoveryCode < ApplicationRecord
  belongs_to :user

  COUNT = 10

  scope :unused, -> { where(used_at: nil) }

  # Replace the user's codes with a fresh set. Returns the human-readable codes
  # (grouped "abcde-fghij") for one-time display; only digests are persisted.
  def self.regenerate_for!(user)
    raws = Array.new(COUNT) { SecureRandom.alphanumeric(10).downcase }
    transaction do
      user.recovery_codes.delete_all
      raws.each { |raw| user.recovery_codes.create!(code_digest: BCrypt::Password.create(raw)) }
    end
    raws.map { |raw| "#{raw[0, 5]}-#{raw[5, 5]}" }
  end

  # Spend a code: find the matching unused one (constant-time bcrypt compare),
  # stamp it used, and return it. Returns nil when nothing matches.
  def self.consume!(user, plaintext)
    normalized = normalize(plaintext)
    return nil if normalized.blank?

    user.recovery_codes.unused.find { |rc| rc.matches?(normalized) }&.tap do |rc|
      rc.update!(used_at: Time.current)
    end
  end

  # Strip everything but a-z0-9 so "ABCDE-FGHIJ", "abcde fghij" and "abcdefghij"
  # all reduce to the stored canonical form.
  def self.normalize(input)
    input.to_s.downcase.gsub(/[^a-z0-9]/, "")
  end

  def matches?(normalized)
    BCrypt::Password.new(code_digest) == normalized
  rescue BCrypt::Errors::InvalidHash
    false
  end
end
