class BetaCode < ApplicationRecord
  belongs_to :redeemed_by, class_name: "User", optional: true
  belongs_to :created_by, class_name: "User", optional: true

  validates :code, presence: true, uniqueness: true

  before_validation :generate_code, on: :create

  scope :unredeemed, -> { where(redeemed_at: nil) }
  scope :redeemed, -> { where.not(redeemed_at: nil) }
  scope :chronological, -> { order(created_at: :desc) }
  scope :redeemable, -> { unredeemed.where("expires_at IS NULL OR expires_at > ?", Time.current) }

  # Unambiguous alphabet (no 0/O/1/I/L) so codes are easy to read and type.
  ALPHABET = "ABCDEFGHJKMNPQRSTUVWXYZ23456789".freeze

  # Mint `count` fresh single-use codes. Returns the created records.
  def self.generate_batch(count:, label: nil, created_by: nil)
    n = count.to_i.clamp(1, 200)
    Array.new(n) { create!(label: label.presence, created_by: created_by) }
  end

  # Look up a redeemable code by the string a human typed. Matching ignores case
  # and any punctuation/spacing, so a code shown as "ABCD-2345" still resolves
  # when entered as "abcd2345", "ABCD 2345", or with a pasted smart-dash — the
  # hyphen is presentational, not part of the secret.
  def self.find_redeemable(entered)
    normalized = entered.to_s.upcase.gsub(/[^A-Z0-9]/, "")
    return nil if normalized.blank?

    # Stored codes only ever contain a single hyphen, so strip it DB-side and
    # compare bare alphanumerics. The table is tiny (admin-minted), so the
    # non-indexed scan is irrelevant.
    redeemable.where("REPLACE(UPPER(code), '-', '') = ?", normalized).first
  end

  def self.friendly_code
    block = -> { Array.new(4) { ALPHABET[SecureRandom.random_number(ALPHABET.size)] }.join }
    "#{block.call}-#{block.call}"
  end

  def redeemed?
    redeemed_at.present?
  end

  def expired?
    expires_at.present? && expires_at < Time.current
  end

  def redeemable?
    !redeemed? && !expired?
  end

  def status
    return :redeemed if redeemed?
    return :expired if expired?
    :available
  end

  # Atomically claim this code for `user`. Returns false if it was already
  # taken or expired (e.g. a concurrent signup won the race), so the caller can
  # roll back the surrounding transaction.
  def redeem!(user)
    with_lock do
      return false unless redeemable?
      update!(redeemed_at: Time.current, redeemed_by: user)
    end
    true
  end

  private

  def generate_code
    return if code.present?

    self.code = loop do
      candidate = self.class.friendly_code
      break candidate unless self.class.exists?(code: candidate)
    end
  end
end
